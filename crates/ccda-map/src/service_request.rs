//! Planned and requested activities (mood other than EVN) to FHIR `ServiceRequest`.
//!
//! The IG has no page for these; this follows C-CDA Planned Procedure (4.41), Planned Encounter
//! (4.40), Planned Observation (4.44), Planned Act (4.39) and the same activities written with a
//! non-event mood, plus the US Core ServiceRequest profile. See `docs/decisions/0019-service-requests.md`.

use ccda_model::{ClinicalDocument, Statement, StatementKind, Value as Cv};
use serde_json::{json, Map, Value};

use crate::author::{apply_authors, AuthorFields};
use crate::util::nested;
use crate::{
    data_absent_reason_extension, map_codeable_concept, map_datetime, map_identifier, Context,
};

/// Referral (4.140): an act that asks another provider to see the patient.
const REFERRAL: &str = "2.16.840.1.113883.10.20.22.4.140";
const PLANNED_ACT: &str = "2.16.840.1.113883.10.20.22.4.39";
const PLANNED_ENCOUNTER: &str = "2.16.840.1.113883.10.20.22.4.40";
const PLANNED_PROCEDURE: &str = "2.16.840.1.113883.10.20.22.4.41";
const PLANNED_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.44";
const PROCEDURE_ACTIVITY_PROCEDURE: &str = "2.16.840.1.113883.10.20.22.4.14";
const PROCEDURE_ACTIVITY_ACT: &str = "2.16.840.1.113883.10.20.22.4.12";
const ENCOUNTER_ACTIVITY: &str = "2.16.840.1.113883.10.20.22.4.49";
const INSTRUCTION: &str = "2.16.840.1.113883.10.20.22.4.20";
const PROCEDURE_ACTIVITY_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.13";
const INDICATION: &str = "2.16.840.1.113883.10.20.22.4.19";
const SNOMED_URI: &str = "http://snomed.info/sct";

/// `ServiceRequest.requester` is the latest author; `authoredOn` the earliest author time.
const FIELDS: AuthorFields = AuthorFields {
    recorder: Some("requester"),
    date: "authoredOn",
};

/// Map every planned or requested activity in the document.
pub fn map_service_requests(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind == StatementKind::Encounter
                && s.mood_code.as_deref() == Some("APT")
                && s.has_template(PLANNED_ENCOUNTER)
            {
                out.push(crate::appointment::map_appointment(s, ctx));
            } else if is_planned(s) {
                out.push(map_request(s, ctx));
            }
        }
    }
    out
}

fn is_event(s: &Statement) -> bool {
    matches!(s.mood_code.as_deref(), Some("EVN") | None)
}

fn is_planned(s: &Statement) -> bool {
    let planned_template = (s.kind == StatementKind::Procedure
        && s.has_template(PLANNED_PROCEDURE))
        || (s.kind == StatementKind::Encounter && s.has_template(PLANNED_ENCOUNTER))
        || (s.kind == StatementKind::Observation && s.has_template(PLANNED_OBSERVATION))
        || (s.kind == StatementKind::Act && s.has_template(PLANNED_ACT))
        || (s.kind == StatementKind::Act && s.has_template(REFERRAL));
    let activity_template = (s.kind == StatementKind::Procedure
        && s.has_template(PROCEDURE_ACTIVITY_PROCEDURE))
        || (s.kind == StatementKind::Act && s.has_template(PROCEDURE_ACTIVITY_ACT))
        || (s.kind == StatementKind::Encounter && s.has_template(ENCOUNTER_ACTIVITY));
    // An Instruction is always a request for patient education or instruction.
    if s.kind == StatementKind::Act && s.has_template(INSTRUCTION) {
        return true;
    }
    let activity_template = activity_template
        || (s.kind == StatementKind::Observation && s.has_template(PROCEDURE_ACTIVITY_OBSERVATION));
    // A planned template says "planned" itself; a plain activity template does only through its mood.
    planned_template && !matches!(s.mood_code.as_deref(), Some("EVN"))
        || activity_template && !is_event(s)
}

fn intent(mood: Option<&str>) -> &'static str {
    match mood {
        Some("PRP") => "proposal",
        Some("RQO" | "ARQ" | "APT") => "order",
        // INT (intent), PRMS (promise) and a missing mood are plans.
        _ => "plan",
    }
}

fn status(s: &Statement, ctx: &mut Context) -> &'static str {
    match s.status_code.as_deref() {
        Some("active" | "new") => "active",
        Some("completed") => "completed",
        Some("aborted" | "cancelled") => "revoked",
        Some("held" | "suspended") => "on-hold",
        Some("nullified") => "entered-in-error",
        other => {
            ctx.report.info(
                "servicerequest.status.assumed",
                if other.is_some() {
                    "a statusCode is not a known request status; status was set to active"
                } else {
                    "a planned activity has no statusCode; status was set to active"
                },
            );
            "active"
        }
    }
}

/// A broad SNOMED category from the kind of activity. C-CDA has no category, US Core wants one.
fn category(s: &Statement) -> Option<(&'static str, &'static str)> {
    if s.has_template(INSTRUCTION) {
        return Some(("409073007", "Education"));
    }
    if s.has_template(REFERRAL) {
        return Some(("3457005", "Patient referral"));
    }
    match s.kind {
        StatementKind::Procedure => Some(("71388002", "Procedure")),
        StatementKind::Encounter => Some(("308335008", "Patient encounter procedure")),
        StatementKind::Observation => Some(("386053000", "Evaluation procedure")),
        _ => None,
    }
}

fn map_request(s: &Statement, ctx: &mut Context) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("ServiceRequest"));
    r.insert("id".into(), json!(ctx.next_id("servicerequest")));
    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    let st = status(s, ctx);
    r.insert("status".into(), json!(st));
    r.insert("intent".into(), json!(intent(s.mood_code.as_deref())));
    if let Some((code, display)) = category(s) {
        ctx.report.info(
            "servicerequest.category.assumed",
            "ServiceRequest.category is not in C-CDA; a broad SNOMED category was chosen from the kind of activity",
        );
        r.insert(
            "category".into(),
            json!([{ "coding": [{ "system": SNOMED_URI, "code": code, "display": display }] }]),
        );
    }

    let code = s.code.as_ref().and_then(|c| {
        map_codeable_concept(c, ctx.terminology, ctx.report).or_else(|| {
            c.null_flavor
                .as_deref()
                .map(|nf| json!({ "extension": [data_absent_reason_extension(nf, ctx.report)] }))
        })
    });
    r.insert(
        "code".into(),
        code.unwrap_or_else(|| {
            ctx.report.warn(
                "servicerequest.code.missing",
                "a planned activity has no code; code was marked unknown",
            );
            json!({ "extension": [data_absent_reason_extension("UNK", ctx.report)] })
        }),
    );
    r.insert("subject".into(), json!({ "reference": ctx.patient }));

    if let Some(t) = &s.effective_time {
        let low = t.low.as_ref().and_then(|p| p.value.as_deref());
        let high = t.high.as_ref().and_then(|p| p.value.as_deref());
        if let Some(v) = t.value.as_deref() {
            if let Some(d) = map_datetime(v, ctx.report) {
                r.insert("occurrenceDateTime".into(), json!(d));
            }
        } else if low.is_some() || high.is_some() {
            let mut p = Map::new();
            if let Some(d) = low.and_then(|v| map_datetime(v, ctx.report)) {
                p.insert("start".into(), json!(d));
            }
            if let Some(d) = high.and_then(|v| map_datetime(v, ctx.report)) {
                p.insert("end".into(), json!(d));
            }
            if !p.is_empty() {
                r.insert("occurrencePeriod".into(), Value::Object(p));
            }
        }
    }

    let sites: Vec<Value> = s
        .target_site_codes
        .iter()
        .filter_map(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
        .collect();
    if !sites.is_empty() {
        r.insert("bodySite".into(), json!(sites));
    }

    let reasons: Vec<Value> = nested(s)
        .filter(|n| n.has_template(INDICATION))
        .filter_map(|n| match n.values.first() {
            Some(Cv::Coded(c)) => map_codeable_concept(c, ctx.terminology, ctx.report),
            _ => None,
        })
        .collect();
    if !reasons.is_empty() {
        r.insert("reasonCode".into(), json!(reasons));
    }

    if s.has_template(INSTRUCTION) {
        if let Some(t) = s.text.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            r.insert("patientInstruction".into(), json!(t));
        }
    }

    if let Some(priority) = s.priority_code.as_ref().and_then(|c| c.code.as_deref()) {
        match priority {
            "R" => {
                r.insert("priority".into(), json!("routine"));
            }
            "UR" => {
                r.insert("priority".into(), json!("urgent"));
            }
            "A" => {
                r.insert("priority".into(), json!("asap"));
            }
            "S" | "EM" => {
                r.insert("priority".into(), json!("stat"));
            }
            _ => ctx.report.info(
                "servicerequest.priority.unmapped",
                "a priorityCode has no FHIR request priority; priority was omitted",
            ),
        }
    }

    // The referral recipient (`participant typeCode="REFT"`) is the performer; its role code
    // (a specialty) is the performer type.
    for p in s
        .participants
        .iter()
        .filter(|p| p.type_code.as_deref() == Some("REFT"))
    {
        let person = p.person_name.clone().unwrap_or_default();
        let agent = ccda_model::Author {
            ids: p.ids.clone(),
            role_code: p.role_code.clone(),
            given: person.given,
            family: person.family.or_else(|| p.name.clone()),
            telecoms: p.telecoms.clone(),
            addresses: p.addresses.clone(),
            ..ccda_model::Author::default()
        };
        if let Some(reference) = crate::author::register_author_agent(&agent, ctx) {
            r.insert("performer".into(), json!([{ "reference": reference }]));
        }
        if let Some(role) = p
            .role_code
            .as_ref()
            .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
        {
            r.insert("performerType".into(), role);
        }
    }

    let authors: Vec<_> = s.authors.iter().collect();
    apply_authors(&mut r, FIELDS, &authors, ctx);
    Value::Object(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(kind: &str, template: &str, mood: &str, body: &str) -> (Vec<Value>, Report) {
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section>
  <entry><{kind} classCode="PROC" moodCode="{mood}">
    <templateId root="{template}"/>
    <id root="1.2.3" extension="P-1"/>
    <code code="73761001" codeSystem="2.16.840.1.113883.6.96"/>
    {body}
  </{kind}></entry>
</section></component></structuredBody></component></ClinicalDocument>"#
        );
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        (map_service_requests(&d, &mut ctx), report)
    }

    #[test]
    fn planned_procedure_becomes_a_plan() {
        let (out, _) = run(
            "procedure",
            PLANNED_PROCEDURE,
            "INT",
            r#"<statusCode code="active"/><effectiveTime value="20250301"/>"#,
        );
        let r = &out[0];
        assert_eq!(r["intent"], "plan");
        assert_eq!(r["status"], "active");
        assert_eq!(r["occurrenceDateTime"], "2025-03-01");
        assert_eq!(r["code"]["coding"][0]["code"], "73761001");
        assert_eq!(r["category"][0]["coding"][0]["code"], "71388002");
    }

    #[test]
    fn mood_decides_intent() {
        for (mood, want) in [("RQO", "order"), ("PRP", "proposal"), ("INT", "plan")] {
            let (out, _) = run("procedure", PLANNED_PROCEDURE, mood, "");
            assert_eq!(out[0]["intent"], want, "{mood}");
        }
    }

    #[test]
    fn activity_templates_are_requests_only_when_not_events() {
        let (out, _) = run("procedure", PROCEDURE_ACTIVITY_PROCEDURE, "INT", "");
        assert_eq!(out.len(), 1);
        let (out, _) = run("procedure", PROCEDURE_ACTIVITY_PROCEDURE, "EVN", "");
        assert!(out.is_empty());
    }

    #[test]
    fn missing_status_is_assumed_and_reported() {
        let (out, report) = run("observation", PLANNED_OBSERVATION, "INT", "");
        assert_eq!(out[0]["status"], "active");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "servicerequest.status.assumed"));
    }

    #[test]
    fn an_instruction_is_an_education_request_with_the_text_as_patient_instruction() {
        let (out, _) = run(
            "act",
            INSTRUCTION,
            "INT",
            r#"<statusCode code="completed"/><text>Possible flu-like symptoms for three days.</text>"#,
        );
        assert_eq!(out[0]["category"][0]["coding"][0]["code"], "409073007");
        assert_eq!(
            out[0]["patientInstruction"],
            "Possible flu-like symptoms for three days."
        );
        assert_eq!(out[0]["status"], "completed");
    }

    #[test]
    fn a_planned_procedure_activity_observation_is_a_request() {
        let (out, _) = run("observation", PROCEDURE_ACTIVITY_OBSERVATION, "INT", "");
        assert_eq!(out.len(), 1);
        let (out, _) = run("observation", PROCEDURE_ACTIVITY_OBSERVATION, "EVN", "");
        assert!(out.is_empty());
    }

    #[test]
    fn planned_act_has_no_assumed_category() {
        let (out, _) = run("act", PLANNED_ACT, "INT", "");
        assert!(out[0].get("category").is_none());
    }
}
