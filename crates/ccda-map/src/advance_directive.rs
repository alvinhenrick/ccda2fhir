//! Advance Directive Observation (4.48) and Organizer (4.108) to FHIR `Consent`.
//!
//! The observation says which directive exists (`code`, for example resuscitation status) and
//! its value (for example "do not resuscitate"). `Consent` with scope `adr` is the R4 resource
//! for an advance care directive. The value goes in `provision.code`; the provision has no
//! `type`, because the document does not say permit or deny. See ADR 0044.

use ccda_model::{ClinicalDocument, Statement, StatementKind, Value as Cv};
use serde_json::{json, Map, Value};

use crate::author::{apply_authors, AuthorFields};
use crate::util::nested;
use crate::{
    data_absent_reason_extension, map_codeable_concept, map_datetime, map_identifier, Context,
};

const OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.48";
const ORGANIZER: &str = "2.16.840.1.113883.10.20.22.4.108";
const SCOPE_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/consentscope";

const FIELDS: AuthorFields = AuthorFields {
    recorder: None,
    date: "dateTime",
};

/// Map every Advance Directive Observation, alone or inside an organizer.
pub fn map_advance_directives(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            // The template alone is not enough: a hostile or damaged document can put the
            // template of an observation on any kind of statement.
            if is_directive(s) {
                out.push(map_directive(s, ctx));
            } else if s.has_template(ORGANIZER) && s.kind == StatementKind::Organizer {
                for inner in nested(s).filter(|i| is_directive(i)) {
                    out.push(map_directive(inner, ctx));
                }
            }
        }
    }
    out
}

fn is_directive(s: &Statement) -> bool {
    s.kind == StatementKind::Observation && s.has_template(OBSERVATION)
}

fn status_of(code: Option<&str>) -> Option<&'static str> {
    Some(match code? {
        "completed" | "active" => "active",
        "aborted" | "cancelled" | "suspended" => "inactive",
        "nullified" => "entered-in-error",
        _ => return None,
    })
}

fn map_directive(s: &Statement, ctx: &mut Context) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Consent"));
    r.insert("id".into(), json!(ctx.next_id("consent")));
    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    let status = status_of(s.status_code.as_deref()).unwrap_or_else(|| {
        ctx.report.warn(
            "consent.status.assumed",
            "an advance directive has no usable statusCode; status was set to active",
        );
        "active"
    });
    r.insert("status".into(), json!(status));
    r.insert(
        "scope".into(),
        json!({ "coding": [{ "system": SCOPE_SYSTEM, "code": "adr" }] }),
    );
    let category = s
        .code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report));
    r.insert(
        "category".into(),
        json!([category.unwrap_or_else(|| {
            ctx.report.warn(
                "consent.category.missing",
                "an advance directive has no type code; the category is the generic advance directive code",
            );
            json!({ "coding": [{ "system": "http://loinc.org", "code": "42348-3" }] })
        })]),
    );
    r.insert("patient".into(), json!({ "reference": ctx.patient }));
    // R4 requires a policy or a policyRule (ppc-1). C-CDA names none, so it is marked unknown.
    r.insert(
        "policyRule".into(),
        json!({ "extension": [data_absent_reason_extension("UNK", ctx.report)] }),
    );

    let authors: Vec<_> = s.authors.iter().collect();
    apply_authors(&mut r, FIELDS, &authors, ctx);

    let mut provision = Map::new();
    if let Some(t) = &s.effective_time {
        if let Some(low) = t
            .low
            .as_ref()
            .and_then(|l| l.value.as_deref())
            .or(t.value.as_deref())
            .and_then(|v| map_datetime(v, ctx.report))
        {
            r.insert("dateTime".into(), json!(low));
        }
        let mut period = Map::new();
        if let Some(v) = t
            .low
            .as_ref()
            .and_then(|l| l.value.as_deref())
            .and_then(|v| map_datetime(v, ctx.report))
        {
            period.insert("start".into(), json!(v));
        }
        if let Some(v) = t
            .high
            .as_ref()
            .and_then(|h| h.value.as_deref())
            .and_then(|v| map_datetime(v, ctx.report))
        {
            period.insert("end".into(), json!(v));
        }
        if !period.is_empty() {
            provision.insert("period".into(), Value::Object(period));
        }
    }
    if let Some(Cv::Coded(code)) = s.values.first() {
        if let Some(c) = map_codeable_concept(code, ctx.terminology, ctx.report) {
            provision.insert("code".into(), json!([c]));
        }
    }
    // Participants: a verifier says the directive was checked; a custodian is the healthcare agent.
    let mut verifications: Vec<Value> = Vec::new();
    let mut actors: Vec<Value> = Vec::new();
    let mut other = false;
    for p in &s.participants {
        match p.type_code.as_deref() {
            Some("VRF") => {
                let mut v = Map::new();
                v.insert("verified".into(), json!(true));
                if let Some(d) = p.time.as_deref().and_then(|t| map_datetime(t, ctx.report)) {
                    v.insert("verificationDate".into(), json!(d));
                }
                verifications.push(Value::Object(v));
            }
            Some("CST") => {
                let relationship = p
                    .playing_entity_code
                    .as_ref()
                    .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report));
                match crate::coverage::related_person(p, relationship, ctx) {
                    Some(reference) => {
                        let role = p
                            .role_code
                            .as_ref()
                            .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
                            .unwrap_or_else(|| {
                                json!({ "extension": [data_absent_reason_extension("NI", ctx.report)] })
                            });
                        actors
                            .push(json!({ "role": role, "reference": { "reference": reference } }));
                    }
                    None => other = true,
                }
            }
            _ => other = true,
        }
    }
    if !verifications.is_empty() {
        r.insert("verification".into(), json!(verifications));
        // Consent.verification has no place for the clinician who verified.
        ctx.report.info(
            "consent.verifier.identity-dropped",
            "a verifier of an advance directive is recorded as verified, without who verified it",
        );
    }
    if !actors.is_empty() {
        provision.insert("actor".into(), json!(actors));
    }
    if !provision.is_empty() {
        r.insert("provision".into(), Value::Object(provision));
    }
    if other {
        ctx.report.info(
            "consent.participants.unsupported",
            "a participant of an advance directive that is not a verifier or a named custodian was not mapped",
        );
    }
    Value::Object(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    const DOC: &str = r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section><entry><observation classCode="OBS" moodCode="EVN">
<templateId root="2.16.840.1.113883.10.20.22.4.48"/>
<id root="1.2.3" extension="AD-1"/>
<code code="304251008" codeSystem="2.16.840.1.113883.6.96"/>
<statusCode code="completed"/>
<effectiveTime><low value="20240101"/><high value="20261231"/></effectiveTime>
<value xsi:type="CD" code="304253006" codeSystem="2.16.840.1.113883.6.96"/>
<participant typeCode="VRF"><time value="20240102"/><participantRole><playingEntity><name><family>Verifier</family></name></playingEntity></participantRole></participant>
<participant typeCode="CST"><participantRole classCode="AGNT"><code code="HPOWATT" codeSystem="2.16.840.1.113883.5.111"/>
  <playingEntity><code code="SPS" codeSystem="2.16.840.1.113883.5.111"/><name><given>Pat</given><family>Agent</family></name></playingEntity></participantRole></participant>
<participant typeCode="WIT"><participantRole/></participant>
</observation></entry></section></component></structuredBody></component></ClinicalDocument>"#;

    fn run(xml: &str) -> (Vec<Value>, Report) {
        let doc = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_advance_directives(&doc, &mut ctx);
        (out, report)
    }

    #[test]
    fn directive_becomes_an_adr_consent() {
        let (out, report) = run(DOC);
        let c = &out[0];
        assert_eq!(c["resourceType"], "Consent");
        assert_eq!(c["status"], "active");
        assert_eq!(c["scope"]["coding"][0]["code"], "adr");
        assert_eq!(c["category"][0]["coding"][0]["code"], "304251008");
        assert_eq!(c["provision"]["code"][0]["coding"][0]["code"], "304253006");
        assert_eq!(c["provision"]["period"]["end"], "2026-12-31");
        assert_eq!(c["dateTime"], "2024-01-01");
        assert_eq!(c["verification"][0]["verified"], true);
        assert_eq!(c["verification"][0]["verificationDate"], "2024-01-02");
        let actor = &c["provision"]["actor"][0];
        assert!(actor["reference"]["reference"].is_string());
        assert_eq!(actor["role"]["coding"][0]["code"], "HPOWATT");
        for code in [
            "consent.verifier.identity-dropped",
            "consent.participants.unsupported",
        ] {
            assert!(report.issues.iter().any(|i| i.code == code), "{code}");
        }
    }

    #[test]
    fn missing_type_is_reported() {
        let xml = DOC.replace(
            r#"<code code="304251008" codeSystem="2.16.840.1.113883.6.96"/>"#,
            "",
        );
        let (out, report) = run(&xml);
        assert_eq!(out[0]["category"][0]["coding"][0]["code"], "42348-3");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "consent.category.missing"));
    }

    #[test]
    fn the_directive_template_on_another_kind_of_statement_is_ignored() {
        // Found by fuzzing: an encounter carrying the Advance Directive template.
        let xml = DOC
            .replace("<observation ", "<encounter ")
            .replace("</observation>", "</encounter>");
        let (out, _) = run(&xml);
        assert!(out.is_empty());
    }
}
