//! Health Concern Act (template 4.132) to the `health-concern` category of `Condition`.
//!
//! In FHIR a health concern is a Condition whose category includes `health-concern` (US Core
//! Condition Problems and Health Concerns). In C-CDA the concern act usually only *points at*
//! other entries (Entry Reference, 4.122) or wraps Problem Observations. See
//! `docs/decisions/0022-health-concerns.md`.

use std::collections::BTreeMap;

use ccda_model::{ClinicalDocument, Statement};
use serde_json::{json, Value};

use crate::problem::map_observation;
use crate::{map_identifier, Context, Report};

const HEALTH_CONCERN_ACT: &str = "2.16.840.1.113883.10.20.22.4.132";
const ENTRY_REFERENCE: &str = "2.16.840.1.113883.10.20.22.4.122";
const PROBLEM_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.4";
const US_CORE_CATEGORY_SYSTEM: &str = "http://hl7.org/fhir/us/core/CodeSystem/condition-category";

fn health_concern_category() -> Value {
    json!({ "coding": [{ "system": US_CORE_CATEGORY_SYSTEM, "code": "health-concern" }] })
}

fn has_health_concern_category(condition: &Value) -> bool {
    condition["category"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|c| c["coding"].as_array().into_iter().flatten())
        .any(|c| c["code"] == "health-concern")
}

/// `system|value` keys of a resource's identifiers.
pub(crate) fn keys(identifiers: &Value) -> Vec<String> {
    identifiers
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| {
            Some(format!(
                "{}|{}",
                i["system"].as_str().unwrap_or_default(),
                i["value"].as_str()?
            ))
        })
        .collect()
}

/// Apply every Health Concern Act: mark the Conditions it points at, and map the Problem
/// Observations it wraps. `resources` are the resources already mapped (the Conditions are
/// updated in place); the return value is any new Conditions.
pub fn map_health_concerns(
    doc: &ClinicalDocument,
    ctx: &mut Context,
    resources: &mut [Value],
) -> Vec<Value> {
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for (i, r) in resources.iter().enumerate() {
        if r["resourceType"] == "Condition" {
            for k in keys(&r["identifier"]) {
                index.entry(k).or_insert(i);
            }
        }
    }

    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let concern = &entry.statement;
            if !concern.has_template(HEALTH_CONCERN_ACT) {
                continue;
            }
            if concern.related.is_empty() {
                ctx.report.info(
                    "healthconcern.empty",
                    "a Health Concern Act points at nothing; no health concern was created",
                );
            }
            for related in &concern.related {
                let s = &related.statement;
                if s.has_template(ENTRY_REFERENCE) {
                    reference(s, resources, &index, ctx);
                } else if s.has_template(PROBLEM_OBSERVATION) {
                    out.push(map_observation(
                        Some(health_concern_category()),
                        concern,
                        s,
                        ctx,
                    ));
                } else {
                    ctx.report.info(
                        "healthconcern.nested.unsupported",
                        "a Health Concern Act contains an entry that is neither an Entry Reference nor a Problem Observation; it was not mapped",
                    );
                }
            }
        }
    }
    out
}

fn reference(
    reference: &Statement,
    resources: &mut [Value],
    index: &BTreeMap<String, usize>,
    ctx: &mut Context,
) {
    // Matching must not add report entries of its own.
    let mut scratch = Report::default();
    let found = reference
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, &mut scratch))
        .flat_map(|i| keys(&json!([i])))
        .find_map(|k| index.get(&k).copied());
    let Some(i) = found else {
        ctx.report.info(
            "healthconcern.reference.unresolved",
            "a Health Concern Act points at an entry that was not mapped to a Condition; nothing was marked",
        );
        return;
    };
    if has_health_concern_category(&resources[i]) {
        return;
    }
    if let Some(categories) = resources[i]["category"].as_array_mut() {
        categories.push(health_concern_category());
    } else {
        resources[i]["category"] = json!([health_concern_category()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccda_terminology::BuiltinTerminology;

    fn run(body: &str, conditions: &mut [Value]) -> (Vec<Value>, Report) {
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><component><structuredBody><component><section>
<entry><act classCode="ACT" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.132"/>
  <id root="1.2.3" extension="HC-1"/>
  <code code="75310-3" codeSystem="2.16.840.1.113883.6.1"/>
  <statusCode code="active"/>
  {body}
</act></entry>
</section></component></structuredBody></component></ClinicalDocument>"#
        );
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_health_concerns(&d, &mut ctx, conditions);
        (out, report)
    }

    fn condition() -> Value {
        json!({
            "resourceType": "Condition", "id": "condition-1",
            "identifier": [{ "system": "urn:oid:1.2.3", "value": "P-1" }],
            "category": [{ "coding": [{ "code": "problem-list-item" }] }]
        })
    }

    const POINTER: &str = r#"<entryRelationship typeCode="REFR"><act classCode="ACT" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.122"/>
  <id root="1.2.3" extension="P-1"/><code nullFlavor="NP"/></act></entryRelationship>"#;

    #[test]
    fn a_pointed_at_condition_gains_the_health_concern_category_once() {
        let mut conditions = [condition()];
        let (out, report) = run(POINTER, &mut conditions);
        assert!(out.is_empty());
        let cats = conditions[0]["category"].as_array().unwrap();
        assert_eq!(cats.len(), 2);
        assert_eq!(cats[1]["coding"][0]["code"], "health-concern");
        // Pointing at it twice changes nothing.
        let (_, _) = run(&format!("{POINTER}{POINTER}"), &mut conditions);
        assert_eq!(conditions[0]["category"].as_array().unwrap().len(), 2);
        assert!(report.issues.is_empty());
    }

    #[test]
    fn an_unresolved_pointer_is_reported() {
        let (_, report) = run(POINTER, &mut []);
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "healthconcern.reference.unresolved"));
    }

    #[test]
    fn a_wrapped_problem_observation_becomes_a_health_concern_condition() {
        let body = r#"<entryRelationship typeCode="SUBJ"><observation classCode="OBS" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.4"/>
  <id root="1.2.3" extension="O-9"/>
  <code code="55607006" codeSystem="2.16.840.1.113883.6.96"/>
  <statusCode code="completed"/>
  <effectiveTime><low value="20240101"/></effectiveTime>
  <value xsi:type="CD" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" code="38341003" codeSystem="2.16.840.1.113883.6.96"/>
</observation></entryRelationship>"#;
        let (out, _) = run(body, &mut []);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["resourceType"], "Condition");
        assert_eq!(out[0]["category"][0]["coding"][0]["code"], "health-concern");
    }

    #[test]
    fn an_empty_concern_is_reported() {
        let (out, report) = run("", &mut []);
        assert!(out.is_empty());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "healthconcern.empty"));
    }
}
