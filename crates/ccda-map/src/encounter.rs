//! Encounter Activity (`encounter`) to FHIR `Encounter`.
//!
//! Rules come from the IG page "CF-encounters" and the ConceptMap `CF-EncounterStatus`.
//! The header's `encompassingEncounter` and the merging of the same encounter seen twice are not
//! done yet (see `docs/coverage.md`).

use ccda_model::{ClinicalDocument, Statement, StatementKind, Value as Cv};
use serde_json::{json, Map, Value};

use crate::author::{apply_authors, register_practitioner, AuthorFields};
use crate::problem::map_encounter_diagnosis;
use crate::util::nested;
use crate::{
    data_absent_reason_extension, map_codeable_concept, map_datetime, map_identifier, Context,
};

const ENCOUNTER_ACTIVITY: &str = "2.16.840.1.113883.10.20.22.4.49";
const ENCOUNTER_DIAGNOSIS: &str = "2.16.840.1.113883.10.20.22.4.80";
const INDICATION: &str = "2.16.840.1.113883.10.20.22.4.19";

const ACT_CODE_OID: &str = "2.16.840.1.113883.5.4";
const ACT_CODE_URI: &str = "http://terminology.hl7.org/CodeSystem/v3-ActCode";
const CPT_OID: &str = "2.16.840.1.113883.6.12";

/// `Encounter` has no recorder or recorded date; authors only feed Provenance.
const FIELDS: AuthorFields = AuthorFields {
    recorder: None,
    date: "_authored",
};

/// The v3 ActCode values FHIR's encounter class value set uses.
const CLASS_CODES: [&str; 11] = [
    "AMB", "EMER", "FLD", "HH", "IMP", "ACUTE", "NONAC", "OBSENC", "PRENC", "SS", "VR",
];

/// Map every Encounter Activity, with the Conditions of its Encounter Diagnoses.
pub fn map_encounters(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind != StatementKind::Encounter || !s.has_template(ENCOUNTER_ACTIVITY) {
                continue;
            }
            // Planned encounters are ServiceRequests (see `service_request.rs`).
            if s.mood_code.as_deref().is_some_and(|m| m != "EVN") {
                continue;
            }
            map_activity(s, ctx, &mut out);
        }
    }
    out
}

/// `CF-EncounterStatus`.
fn status_of(code: &str) -> Option<&'static str> {
    Some(match code {
        "completed" => "finished",
        "active" => "in-progress",
        "cancelled" => "cancelled",
        "held" => "planned",
        "suspended" => "onleave",
        "nullified" => "entered-in-error",
        "aborted" | "new" => "unknown",
        _ => return None,
    })
}

/// The v3 ActCode class for a CPT evaluation and management code, per the IG's examples.
fn class_from_cpt(code: &str) -> Option<&'static str> {
    let n: u32 = code.parse().ok()?;
    Some(match n {
        99201..=99215 => "AMB",
        99221..=99223 => "IMP",
        99281..=99285 => "EMER",
        _ => return None,
    })
}

/// The class code, and the ActCode value it came from (so `type` can leave it out).
fn class_of(s: &Statement) -> Option<(&'static str, Option<String>)> {
    let code = s.code.as_ref()?;
    let all = std::iter::once(code).chain(code.translations.iter());
    let mut cpt: Option<&'static str> = None;
    for c in all {
        let (Some(value), Some(system)) = (c.code.as_deref(), c.code_system.as_deref()) else {
            continue;
        };
        if system == ACT_CODE_OID {
            if let Some(class) = CLASS_CODES.iter().find(|k| **k == value) {
                return Some((class, Some(value.to_owned())));
            }
        } else if system == CPT_OID && cpt.is_none() {
            cpt = class_from_cpt(value);
        }
    }
    cpt.map(|c| (c, None))
}

fn status(s: &Statement, ctx: &mut Context) -> &'static str {
    let from_code = s.status_code.as_deref();
    // The IG: the statusCode map is for codes other than completed/active, or when the time is
    // missing or ambiguous; otherwise the effective time decides.
    if let Some(code) = from_code.filter(|c| !matches!(*c, "completed" | "active")) {
        return status_of(code).unwrap_or_else(|| {
            ctx.report.warn(
                "encounter.status.unmapped",
                "a statusCode is not in the IG's status map; status was set to unknown",
            );
            "unknown"
        });
    }
    if let Some(t) = &s.effective_time {
        if t.value.is_some() || t.high.as_ref().is_some_and(|h| h.value.is_some()) {
            return "finished";
        }
        if t.low.as_ref().is_some_and(|l| l.value.is_some()) {
            return "in-progress";
        }
    }
    from_code.and_then(status_of).unwrap_or_else(|| {
        ctx.report.warn(
            "encounter.status.missing",
            "an encounter has no usable statusCode or time; status was set to unknown",
        );
        "unknown"
    })
}

fn map_activity(s: &Statement, ctx: &mut Context, out: &mut Vec<Value>) {
    let mut r = Map::new();
    let id = ctx.next_id("encounter");
    r.insert("resourceType".into(), json!("Encounter"));
    r.insert("id".into(), json!(id));
    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|i| map_identifier(i, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    r.insert("status".into(), json!(status(s, ctx)));

    // class (a Coding) and type (the remaining codes)
    let class = class_of(s);
    match &class {
        Some((code, _)) => {
            r.insert(
                "class".into(),
                json!({ "system": ACT_CODE_URI, "code": code }),
            );
        }
        None => {
            ctx.report.warn(
                "encounter.class.missing",
                "an encounter has no code that maps to a class; class was marked unknown",
            );
            r.insert(
                "class".into(),
                json!({ "extension": [data_absent_reason_extension("UNK", ctx.report)] }),
            );
        }
    }
    if let Some(mut ty) = s
        .code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
    {
        let used = class.as_ref().and_then(|(_, v)| v.clone());
        if let Some(codings) = ty["coding"].as_array_mut() {
            codings.retain(|c| {
                !(c["system"] == ACT_CODE_URI && used.as_deref() == c["code"].as_str())
            });
            if codings.is_empty() {
                ty.as_object_mut().map(|o| o.remove("coding"));
            }
        }
        if ty.as_object().is_some_and(|o| !o.is_empty()) {
            r.insert("type".into(), json!([ty]));
        }
    }

    // period
    if let Some(t) = &s.effective_time {
        let mut period = Map::new();
        let start = t
            .value
            .as_deref()
            .or_else(|| t.low.as_ref().and_then(|l| l.value.as_deref()));
        let end = t
            .value
            .as_deref()
            .or_else(|| t.high.as_ref().and_then(|h| h.value.as_deref()));
        for (key, raw) in [("start", start), ("end", end)] {
            if let Some(d) = raw.and_then(|raw| map_datetime(raw, ctx.report)) {
                period.insert(key.into(), json!(d));
            }
        }
        if !period.is_empty() {
            r.insert("period".into(), Value::Object(period));
        }
    }
    r.insert("subject".into(), json!({ "reference": ctx.patient }));

    if let Some(d) = s
        .discharge_disposition
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
    {
        r.insert(
            "hospitalization".into(),
            json!({ "dischargeDisposition": d }),
        );
    }

    let mut participants: Vec<Value> = Vec::new();
    for performer in &s.performers {
        if let Some(reference) = register_practitioner(performer, ctx) {
            let entry = json!({ "individual": { "reference": reference } });
            if !participants.contains(&entry) {
                participants.push(entry);
            }
        }
    }
    if !participants.is_empty() {
        r.insert("participant".into(), json!(participants));
    }
    let locations: Vec<Value> = s
        .participants
        .iter()
        .filter(|p| p.type_code.as_deref() == Some("LOC"))
        .filter_map(|p| crate::location::register_location(p, ctx))
        .map(|reference| json!({ "location": { "reference": reference } }))
        .collect();
    if !locations.is_empty() {
        r.insert("location".into(), json!(locations));
    }

    let encounter_ref = ctx.reference("Encounter", &id);
    let mut reasons: Vec<Value> = Vec::new();
    let mut diagnoses: Vec<Value> = Vec::new();
    let mut conditions: Vec<Value> = Vec::new();
    for related in nested(s) {
        if related.has_template(INDICATION) {
            if let Some(Cv::Coded(code)) = related.values.first() {
                if let Some(c) = map_codeable_concept(code, ctx.terminology, ctx.report) {
                    reasons.push(c);
                }
            }
        } else if related.has_template(ENCOUNTER_DIAGNOSIS) {
            for condition in map_encounter_diagnosis(related, &encounter_ref, ctx) {
                if let Some(cid) = condition["id"].as_str() {
                    diagnoses.push(
                        json!({ "condition": { "reference": ctx.reference("Condition", cid) } }),
                    );
                }
                conditions.push(condition);
            }
        }
    }
    if !reasons.is_empty() {
        r.insert("reasonCode".into(), json!(reasons));
    }
    if !diagnoses.is_empty() {
        r.insert("diagnosis".into(), json!(diagnoses));
    }

    let authors: Vec<_> = s.authors.iter().collect();
    apply_authors(&mut r, FIELDS, &authors, ctx);
    r.remove("_authored");
    out.push(Value::Object(r));
    out.extend(conditions);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn doc(code: &str, body: &str) -> String {
        format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:sdtc="urn:hl7-org:sdtc">
<component><structuredBody><component><section>
  <code code="46240-8" codeSystem="2.16.840.1.113883.6.1"/>
  <entry><encounter classCode="ENC" moodCode="EVN">
    <templateId root="2.16.840.1.113883.10.20.22.4.49"/>
    <id root="1.2.3" extension="E-1"/>
    {code}
    {body}
  </encounter></entry>
</section></component></structuredBody></component></ClinicalDocument>"#
        )
    }

    fn run(xml: &str) -> (Vec<Value>, Report) {
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_encounters(&d, &mut ctx);
        (out, report)
    }

    const CPT: &str = r#"<code code="99213" codeSystem="2.16.840.1.113883.6.12"/>"#;

    #[test]
    fn cpt_gives_class_and_stays_in_type() {
        for (cpt, class) in [("99213", "AMB"), ("99222", "IMP"), ("99283", "EMER")] {
            let (out, _) = run(&doc(
                &format!(r#"<code code="{cpt}" codeSystem="2.16.840.1.113883.6.12"/>"#),
                "",
            ));
            assert_eq!(out[0]["class"]["code"], class);
            assert_eq!(out[0]["type"][0]["coding"][0]["code"], cpt);
        }
    }

    #[test]
    fn act_code_gives_class_and_leaves_type() {
        let code = r#"<code code="AMB" codeSystem="2.16.840.1.113883.5.4"><translation code="99213" codeSystem="2.16.840.1.113883.6.12"/></code>"#;
        let (out, _) = run(&doc(code, ""));
        assert_eq!(out[0]["class"]["code"], "AMB");
        let codings = out[0]["type"][0]["coding"].as_array().unwrap();
        assert_eq!(codings.len(), 1);
        assert_eq!(codings[0]["code"], "99213");
    }

    #[test]
    fn unknown_class_is_marked_unknown() {
        let (out, report) = run(&doc(r#"<code code="12345" codeSystem="1.2.3"/>"#, ""));
        assert_eq!(out[0]["class"]["extension"][0]["valueCode"], "unknown");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "encounter.class.missing"));
    }

    #[test]
    fn time_decides_status_unless_the_status_code_says_otherwise() {
        let point = r#"<effectiveTime value="20240101"/>"#;
        let open = r#"<effectiveTime><low value="20240101"/></effectiveTime>"#;
        let (out, _) = run(&doc(CPT, point));
        assert_eq!(out[0]["status"], "finished");
        let (out, _) = run(&doc(CPT, open));
        assert_eq!(out[0]["status"], "in-progress");
        let (out, _) = run(&doc(
            CPT,
            &format!(r#"<statusCode code="completed"/>{open}"#),
        ));
        assert_eq!(out[0]["status"], "in-progress");
        let (out, _) = run(&doc(
            CPT,
            &format!(r#"<statusCode code="suspended"/>{point}"#),
        ));
        assert_eq!(out[0]["status"], "onleave");
        let (out, _) = run(&doc(CPT, r#"<statusCode code="completed"/>"#));
        assert_eq!(out[0]["status"], "finished");
        let (out, report) = run(&doc(CPT, ""));
        assert_eq!(out[0]["status"], "unknown");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "encounter.status.missing"));
    }

    #[test]
    fn point_time_is_a_period_with_both_ends() {
        let (out, _) = run(&doc(CPT, r#"<effectiveTime value="20240101"/>"#));
        assert_eq!(out[0]["period"]["start"], "2024-01-01");
        assert_eq!(out[0]["period"]["end"], "2024-01-01");
    }

    #[test]
    fn discharge_disposition_is_read_from_the_sdtc_element() {
        let body =
            r#"<sdtc:dischargeDispositionCode code="01" codeSystem="2.16.840.1.113883.12.112"/>"#;
        let (out, _) = run(&doc(CPT, body));
        assert_eq!(
            out[0]["hospitalization"]["dischargeDisposition"]["coding"][0]["code"],
            "01"
        );
    }

    #[test]
    fn encounter_diagnoses_become_conditions_linked_both_ways() {
        let body = r#"<entryRelationship typeCode="REFR"><act classCode="ACT" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.80"/>
  <code code="29308-4" codeSystem="2.16.840.1.113883.6.1"/>
  <entryRelationship typeCode="SUBJ"><observation classCode="OBS" moodCode="EVN">
    <templateId root="2.16.840.1.113883.10.20.22.4.4"/>
    <id root="1.2.3" extension="D-1"/>
    <code code="282291009" codeSystem="2.16.840.1.113883.6.96"/>
    <value xsi:type="CD" code="38341003" codeSystem="2.16.840.1.113883.6.96"/>
  </observation></entryRelationship>
</act></entryRelationship>"#;
        let (out, _) = run(&doc(CPT, body));
        assert_eq!(out.len(), 2);
        let condition = &out[1];
        assert_eq!(condition["resourceType"], "Condition");
        assert_eq!(
            condition["category"][0]["coding"][0]["code"],
            "encounter-diagnosis"
        );
        assert!(condition["encounter"]["reference"].is_string());
        assert_eq!(
            out[0]["diagnosis"][0]["condition"]["reference"]
                .as_str()
                .unwrap()
                .len(),
            45
        );
    }
}
