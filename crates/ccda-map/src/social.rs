//! Social History Observations (including smoking status and pregnancy) to FHIR `Observation`.
//!
//! Rules come from the IG page "CF-social". Birth sex, gender identity and tribal affiliation
//! are Patient extensions in the IG, not Observations; they are reported until the fuller
//! Patient demographics mapper exists.

use ccda_model::{ClinicalDocument, Statement, StatementKind, Value as Cv};
use serde_json::{json, Value};

use crate::observation::{apply_performer, base, category};
use crate::util::nested;
use crate::{map_datetime, Context};

const SOCIAL_HISTORY: &str = "2.16.840.1.113883.10.20.22.4.38";
const SMOKING_STATUS: &str = "2.16.840.1.113883.10.20.22.4.78";
const TOBACCO_USE: &str = "2.16.840.1.113883.10.20.22.4.85";
const PREGNANCY: &str = "2.16.840.1.113883.10.20.15.3.8";
const EDD: &str = "2.16.840.1.113883.10.20.15.3.1";
const BIRTH_SEX: &str = "2.16.840.1.113883.10.20.22.4.200";
const GENDER_IDENTITY: &str = "2.16.840.1.113883.10.20.34.3.45";

const LOINC_URI: &str = "http://loinc.org";
const NULL_FLAVOR_URI: &str = "http://terminology.hl7.org/CodeSystem/v3-NullFlavor";

/// Map every social history observation in the document.
pub fn map_social_history(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind != StatementKind::Observation {
                continue;
            }
            if s.has_template(BIRTH_SEX) || s.has_template(GENDER_IDENTITY) {
                ctx.report.info(
                    "social.patient-extension.unsupported",
                    "birth sex or gender identity belongs on the Patient and was not mapped as an Observation",
                );
            } else if s.has_template(PREGNANCY) {
                out.push(pregnancy(s, ctx));
            } else if s.has_template(SOCIAL_HISTORY)
                || s.has_template(SMOKING_STATUS)
                || s.has_template(TOBACCO_USE)
            {
                out.push(social(s, ctx));
            }
        }
    }
    out
}

fn social(s: &Statement, ctx: &mut Context) -> Value {
    let mut r = base(s, "social", ctx);
    r.insert(
        "category".into(),
        category("social-history", "Social History"),
    );
    // Social history prefers a single date: a start with no end is a date, not a period.
    if let Some(start) = r
        .get("effectivePeriod")
        .filter(|p| p.get("end").is_none())
        .and_then(|p| p.get("start"))
        .cloned()
    {
        r.remove("effectivePeriod");
        r.insert("effectiveDateTime".into(), start);
    }
    apply_performer(&mut r, s, None, ctx);
    Value::Object(r)
}

fn pregnancy(s: &Statement, ctx: &mut Context) -> Value {
    let mut shell = s.clone();
    shell.effective_time = None;
    let mut r = base(&shell, "social", ctx);
    r.insert(
        "category".into(),
        category("social-history", "Social History"),
    );
    // Before C-CDA 4.0 the code is ASSERTION; the FHIR profiles use LOINC.
    let intent = s.mood_code.as_deref() == Some("INT");
    let (code, display) = if intent {
        ("86645-9", "Pregnancy intention in the next year")
    } else {
        ("82810-3", "Pregnancy status")
    };
    r.insert(
        "code".into(),
        json!({ "coding": [{ "system": LOINC_URI, "code": code, "display": display }] }),
    );

    // FHIR allows one time: the first populated of value, low, high, then author time.
    let t = s.effective_time.as_ref();
    let candidates = [
        t.and_then(|t| t.value.as_deref()),
        t.and_then(|t| t.low.as_ref())
            .and_then(|p| p.value.as_deref()),
        t.and_then(|t| t.high.as_ref())
            .and_then(|p| p.value.as_deref()),
        s.authors.iter().find_map(|a| a.time.as_deref()),
    ];
    if let Some(d) = candidates
        .into_iter()
        .flatten()
        .find_map(|raw| map_datetime(raw, ctx.report))
    {
        r.insert("effectiveDateTime".into(), json!(d));
    }

    // An unknown value becomes a real coded value, per the IG.
    if matches!(s.values.first(), Some(Cv::Coded(c)) if c.null_flavor.as_deref() == Some("UNK")) {
        r.remove("dataAbsentReason");
        r.insert(
            "valueCodeableConcept".into(),
            json!({ "coding": [{ "system": NULL_FLAVOR_URI, "code": "UNK" }] }),
        );
    }

    let mut components = Vec::new();
    for related in nested(s) {
        if related.has_template(EDD) {
            if let Some(Cv::Timestamp(x)) = related.values.first() {
                if let Some(d) = x.value.as_deref().and_then(|v| map_datetime(v, ctx.report)) {
                    components.push(json!({
                        "code": { "coding": [{ "system": LOINC_URI, "code": "11778-8", "display": "Estimated date of delivery" }] },
                        "valueDateTime": d
                    }));
                }
            }
        }
    }
    if !components.is_empty() {
        r.insert("component".into(), json!(components));
    }
    apply_performer(&mut r, s, None, ctx);
    Value::Object(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn doc(template: &str, attrs: &str, body: &str) -> String {
        format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section>
  <code code="29762-2" codeSystem="2.16.840.1.113883.6.1"/>
  <entry><observation classCode="OBS" moodCode="EVN" {attrs}>
    <templateId root="{template}"/>
    <id root="1.2.3" extension="S-1"/>
    <statusCode code="completed"/>
    {body}
  </observation></entry>
</section></component></structuredBody></component></ClinicalDocument>"#
        )
    }

    fn run(xml: &str) -> (Vec<Value>, Report) {
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_social_history(&d, &mut ctx);
        (out, report)
    }

    #[test]
    fn smoking_status_is_a_social_history_observation_with_a_date() {
        let body = r#"<code code="72166-2" codeSystem="2.16.840.1.113883.6.1"/>
<effectiveTime><low value="20240101"/></effectiveTime>
<value xsi:type="CD" code="8517006" codeSystem="2.16.840.1.113883.6.96"/>"#;
        let (out, _) = run(&doc(SMOKING_STATUS, "", body));
        assert_eq!(out[0]["category"][0]["coding"][0]["code"], "social-history");
        assert_eq!(out[0]["effectiveDateTime"], "2024-01-01");
        assert!(out[0].get("effectivePeriod").is_none());
        assert_eq!(out[0]["status"], "final");
        assert_eq!(
            out[0]["valueCodeableConcept"]["coding"][0]["code"],
            "8517006"
        );
    }

    #[test]
    fn a_real_period_stays_a_period() {
        let body = r#"<code code="11367-0" codeSystem="2.16.840.1.113883.6.1"/>
<effectiveTime><low value="20200101"/><high value="20230101"/></effectiveTime>
<value xsi:type="ST">pack a day</value>"#;
        let (out, _) = run(&doc(TOBACCO_USE, "", body));
        assert_eq!(out[0]["effectivePeriod"]["end"], "2023-01-01");
        assert_eq!(out[0]["valueString"], "pack a day");
    }

    #[test]
    fn pregnancy_uses_the_loinc_code_one_time_and_an_edd_component() {
        let body = r#"<code code="ASSERTION" codeSystem="2.16.840.1.113883.5.4"/>
<effectiveTime><low nullFlavor="UNK"/><high value="20240901"/></effectiveTime>
<value xsi:type="CD" code="77386006" codeSystem="2.16.840.1.113883.6.96"/>
<entryRelationship typeCode="REFR"><observation classCode="OBS" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.15.3.1"/>
  <code code="11778-8" codeSystem="2.16.840.1.113883.6.1"/>
  <value xsi:type="TS" value="20241201"/>
</observation></entryRelationship>"#;
        let (out, _) = run(&doc(PREGNANCY, "", body));
        assert_eq!(out[0]["code"]["coding"][0]["code"], "82810-3");
        assert_eq!(out[0]["effectiveDateTime"], "2024-09-01");
        assert_eq!(out[0]["component"][0]["valueDateTime"], "2024-12-01");
    }

    #[test]
    fn unknown_pregnancy_value_is_a_null_flavor_coding() {
        let body = r#"<code code="82810-3" codeSystem="2.16.840.1.113883.6.1"/>
<effectiveTime value="20240101"/><value xsi:type="CD" nullFlavor="UNK"/>"#;
        let (out, _) = run(&doc(PREGNANCY, "", body));
        assert_eq!(out[0]["valueCodeableConcept"]["coding"][0]["code"], "UNK");
        assert!(out[0].get("dataAbsentReason").is_none());
    }

    #[test]
    fn birth_sex_is_left_for_the_patient_and_reported() {
        let (out, report) = run(&doc(BIRTH_SEX, "", ""));
        assert!(out.is_empty());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "social.patient-extension.unsupported"));
    }
}
