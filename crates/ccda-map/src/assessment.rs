//! Functional, cognitive and mental status and health status to FHIR `Observation`.
//!
//! No IG page covers these; this follows C-CDA and the US Core Simple Observation profile. An
//! organizer becomes a panel Observation with `hasMember` links to one Observation per component.
//! See `docs/decisions/0023-status-assessments.md`.

use ccda_model::{ClinicalDocument, Statement, StatementKind};
use serde_json::{json, Map, Value};

use crate::observation::{apply_performer, base, insert_effective, insert_span, status};
use crate::{map_identifier, Context};

const US_CORE_CATEGORY: &str = "http://hl7.org/fhir/us/core/CodeSystem/us-core-category";
const OBSERVATION_CATEGORY: &str = "http://terminology.hl7.org/CodeSystem/observation-category";

/// Which assessment a template belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Functional,
    Cognitive,
    Mental,
    Health,
}

/// (template, kind, is an organizer)
const TEMPLATES: &[(&str, Kind, bool)] = &[
    ("2.16.840.1.113883.10.20.22.4.66", Kind::Functional, true), // Functional Status Organizer
    ("2.16.840.1.113883.10.20.22.4.67", Kind::Functional, false), // Functional Status Observation
    ("2.16.840.1.113883.10.20.22.4.68", Kind::Functional, false), // Functional Status Problem Observation
    ("2.16.840.1.113883.10.20.22.4.128", Kind::Functional, false), // Self-Care Activities
    ("2.16.840.1.113883.10.20.22.4.75", Kind::Cognitive, true),   // Cognitive Status Organizer
    ("2.16.840.1.113883.10.20.22.4.74", Kind::Cognitive, false),  // Cognitive Status Observation
    ("2.16.840.1.113883.10.20.22.4.285", Kind::Mental, true),     // Mental Status Organizer
    ("2.16.840.1.113883.10.20.22.4.284", Kind::Mental, false),    // Mental Status Observation
    ("2.16.840.1.113883.10.20.22.4.125", Kind::Mental, false), // Mental Status Problem Observation
    ("2.16.840.1.113883.10.20.22.4.5", Kind::Health, false),   // Health Status Observation
];

fn kind_of(s: &Statement) -> Option<(Kind, bool)> {
    TEMPLATES
        .iter()
        .find(|(t, _, _)| s.has_template(t))
        .map(|(_, k, org)| (*k, *org))
}

/// The templates of this family, for the "not mapped" check.
pub(crate) fn handled_templates() -> impl Iterator<Item = &'static str> {
    TEMPLATES.iter().map(|(t, _, _)| *t)
}

fn category(kind: Kind, ctx: &mut Context) -> Value {
    let (system, code) = match kind {
        Kind::Functional => (US_CORE_CATEGORY, "functional-status"),
        Kind::Cognitive => (US_CORE_CATEGORY, "cognitive-status"),
        Kind::Mental => {
            ctx.report.info(
                "assessment.category.assumed",
                "mental status was given the cognitive-status category; US Core has no separate mental status category",
            );
            (US_CORE_CATEGORY, "cognitive-status")
        }
        Kind::Health => {
            ctx.report.info(
                "assessment.category.assumed",
                "health status has no US Core category; the survey category was used",
            );
            (OBSERVATION_CATEGORY, "survey")
        }
    };
    json!([{ "coding": [{ "system": system, "code": code }] }])
}

/// Map every functional, cognitive, mental and health status entry in the document.
pub fn map_assessments(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            let Some((kind, organizer)) = kind_of(s) else {
                continue;
            };
            if organizer && s.kind == StatementKind::Organizer {
                map_organizer(s, kind, ctx, &mut out);
            } else if !organizer && s.kind == StatementKind::Observation {
                let mut r = observation(s, kind, None, ctx);
                apply_performer(&mut r, s, None, ctx);
                out.push(Value::Object(r));
            }
        }
    }
    out
}

fn observation(
    s: &Statement,
    kind: Kind,
    organizer: Option<&Statement>,
    ctx: &mut Context,
) -> Map<String, Value> {
    let mut r = base(s, "assessment", ctx);
    r.insert("category".into(), category(kind, ctx));
    if organizer.is_some() {
        apply_performer(&mut r, s, organizer, ctx);
    }
    r
}

fn map_organizer(org: &Statement, kind: Kind, ctx: &mut Context, out: &mut Vec<Value>) {
    let mut members: Vec<Value> = Vec::new();
    for related in &org.related {
        let m = &related.statement;
        if m.kind == StatementKind::Observation {
            members.push(Value::Object(observation(m, kind, Some(org), ctx)));
        } else {
            ctx.report.info(
                "assessment.member.unsupported",
                "an assessment organizer contains a component that is not an observation; it was not mapped",
            );
        }
    }

    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Observation"));
    r.insert("id".into(), json!(ctx.next_id("assessment")));
    let identifiers: Vec<Value> = org
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    r.insert("status".into(), json!(status(org, ctx)));
    r.insert("category".into(), category(kind, ctx));
    let code = org
        .code
        .as_ref()
        .and_then(|c| crate::map_codeable_concept(c, ctx.terminology, ctx.report));
    r.insert(
        "code".into(),
        code.unwrap_or_else(|| {
            ctx.report.warn(
                "assessment.code.missing",
                "an assessment organizer has no code; code was marked unknown",
            );
            json!({ "extension": [crate::data_absent_reason_extension("UNK", ctx.report)] })
        }),
    );
    r.insert("subject".into(), json!({ "reference": ctx.patient }));
    insert_effective(&mut r, org, ctx);
    if !r.contains_key("effectiveDateTime") && !r.contains_key("effectivePeriod") {
        insert_span(&mut r, &members);
    }
    let refs: Vec<Value> = members
        .iter()
        .filter_map(|m| m["id"].as_str())
        .map(|id| json!({ "reference": ctx.reference("Observation", id) }))
        .collect();
    if !refs.is_empty() {
        r.insert("hasMember".into(), json!(refs));
    }
    apply_performer(&mut r, org, None, ctx);
    out.push(Value::Object(r));
    out.extend(members);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(entry: &str) -> (Vec<Value>, Report) {
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section>{entry}</section></component></structuredBody></component></ClinicalDocument>"#
        );
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        (map_assessments(&d, &mut ctx), report)
    }

    const FUNCTIONAL: &str = r#"<entry><organizer classCode="CLUSTER" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.66"/>
  <id root="1.2.3" extension="ORG-1"/>
  <code code="d5" codeSystem="2.16.840.1.113883.6.254"/>
  <statusCode code="completed"/>
  <component><observation classCode="OBS" moodCode="EVN">
    <templateId root="2.16.840.1.113883.10.20.22.4.67"/>
    <id root="1.2.3" extension="OBS-1"/>
    <code code="54522-8" codeSystem="2.16.840.1.113883.6.1"/>
    <statusCode code="completed"/>
    <effectiveTime value="20050501"/>
    <value xsi:type="CD" code="105504002" codeSystem="2.16.840.1.113883.6.96"/>
  </observation></component>
  <component><observation classCode="OBS" moodCode="EVN">
    <templateId root="2.16.840.1.113883.10.20.22.4.128"/>
    <id root="1.2.3" extension="OBS-2"/>
    <code nullFlavor="NA"/>
    <statusCode code="completed"/>
    <effectiveTime value="20050501"/>
    <value xsi:type="CD" code="371153006" codeSystem="2.16.840.1.113883.6.96"/>
  </observation></component>
</organizer></entry>"#;

    #[test]
    fn organizer_is_a_panel_with_members_and_the_functional_status_category() {
        let (out, _) = run(FUNCTIONAL);
        assert_eq!(out.len(), 3);
        let panel = &out[0];
        assert_eq!(panel["hasMember"].as_array().unwrap().len(), 2);
        for r in &out {
            assert_eq!(r["category"][0]["coding"][0]["code"], "functional-status");
        }
        assert_eq!(
            out[1]["valueCodeableConcept"]["coding"][0]["code"],
            "105504002"
        );
        // A null-flavored code becomes a data-absent reason, not a missing code.
        assert!(out[2]["code"]["extension"].is_array());
        assert_eq!(panel["effectiveDateTime"], "2005-05-01");
    }

    #[test]
    fn cognitive_and_mental_and_health_status_categories() {
        let obs = |t: &str| {
            format!(
                r#"<entry><observation classCode="OBS" moodCode="EVN"><templateId root="{t}"/><id root="1.2.3" extension="X"/>
<code code="373930000" codeSystem="2.16.840.1.113883.6.96"/><statusCode code="completed"/></observation></entry>"#
            )
        };
        let (out, _) = run(&obs("2.16.840.1.113883.10.20.22.4.74"));
        assert_eq!(
            out[0]["category"][0]["coding"][0]["code"],
            "cognitive-status"
        );
        let (out, report) = run(&obs("2.16.840.1.113883.10.20.22.4.284"));
        assert_eq!(
            out[0]["category"][0]["coding"][0]["code"],
            "cognitive-status"
        );
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "assessment.category.assumed"));
        let (out, _) = run(&obs("2.16.840.1.113883.10.20.22.4.5"));
        assert_eq!(out[0]["category"][0]["coding"][0]["code"], "survey");
    }

    #[test]
    fn unrelated_entries_are_ignored() {
        let (out, _) = run(
            r#"<entry><observation classCode="OBS" moodCode="EVN"><templateId root="2.16.840.1.113883.10.20.22.4.2"/></observation></entry>"#,
        );
        assert!(out.is_empty());
    }
}
