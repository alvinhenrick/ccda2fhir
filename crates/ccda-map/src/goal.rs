//! Goal Observation (template 4.121) to FHIR `Goal`.
//!
//! No IG page covers goals; this follows C-CDA, the US Core Goal profile and common practice.
//! See `docs/decisions/0014-goals.md`.

use ccda_model::{ClinicalDocument, Statement, StatementKind, Value as Cv};
use serde_json::{json, Map, Value};

use std::collections::BTreeMap;

use crate::author::{apply_authors, AuthorFields};
use crate::encounter_link::statement_keys;
use crate::health_concern::keys;
use crate::{map_codeable_concept, map_datetime, map_identifier, Context};

const GOAL_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.121";
const PROGRESS_TOWARD_GOAL: &str = "2.16.840.1.113883.10.20.22.4.110";
const PRIORITY_PREFERENCE: &str = "2.16.840.1.113883.10.20.22.4.143";
const ACT_CODE_OID: &str = "2.16.840.1.113883.5.4";

/// `Goal.expressedBy` holds the latest author. Goal has no recorded date.
const FIELDS: AuthorFields = AuthorFields {
    recorder: Some("expressedBy"),
    date: "_authored",
};

/// Map every Goal Observation in the document.
pub fn map_goals(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind == StatementKind::Observation && s.has_template(GOAL_OBSERVATION) {
                out.push(map_goal(s, ctx));
            }
        }
    }
    out
}

/// Problem Observation: what a goal addresses.
const PROBLEM_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.4";

/// Set `Goal.addresses` to the Conditions that a goal's entry names with a nested Problem
/// Observation. Matching is by identifier; an entry that names a problem outside the document is
/// reported and left unlinked.
pub fn link_goal_addresses(doc: &ClinicalDocument, ctx: &mut Context, resources: &mut [Value]) {
    let mut conditions: BTreeMap<String, String> = BTreeMap::new();
    let mut goals: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, r) in resources.iter().enumerate() {
        match r["resourceType"].as_str().unwrap_or_default() {
            "Condition" => {
                if let Some(id) = r["id"].as_str() {
                    for k in keys(&r["identifier"]) {
                        conditions
                            .entry(k)
                            .or_insert_with(|| ctx.reference("Condition", id));
                    }
                }
            }
            "Goal" => {
                for k in keys(&r["identifier"]) {
                    goals.entry(k).or_default().push(i);
                }
            }
            _ => {}
        }
    }
    if goals.is_empty() {
        return;
    }
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind != StatementKind::Observation || !s.has_template(GOAL_OBSERVATION) {
                continue;
            }
            let own = statement_keys(s, ctx);
            for problem in s
                .related
                .iter()
                .map(|x| &x.statement)
                .filter(|x| x.has_template(PROBLEM_OBSERVATION))
            {
                let reference = statement_keys(problem, ctx)
                    .into_iter()
                    .find_map(|k| conditions.get(&k).cloned());
                let Some(reference) = reference else {
                    ctx.report.info(
                        "goal.addresses.unresolved",
                        "a goal names a problem that is not in the document; no link was made",
                    );
                    continue;
                };
                for k in &own {
                    for &i in goals.get(k).into_iter().flatten() {
                        let list = resources[i]["addresses"].as_array().cloned();
                        let mut list = list.unwrap_or_default();
                        let r = json!({ "reference": reference });
                        if !list.contains(&r) {
                            list.push(r);
                        }
                        resources[i]["addresses"] = json!(list);
                    }
                }
            }
        }
    }
}

fn lifecycle(code: &str) -> Option<&'static str> {
    Some(match code {
        "new" => "proposed",
        "active" => "active",
        "completed" => "completed",
        "suspended" | "held" => "on-hold",
        "aborted" | "cancelled" => "cancelled",
        "nullified" => "entered-in-error",
        _ => return None,
    })
}

/// The goal's description: its code, unless that is the generic ASSERTION, then its value.
fn description(s: &Statement, ctx: &mut Context) -> Option<Value> {
    let from_code = s
        .code
        .as_ref()
        .filter(|c| c.code_system.as_deref() != Some(ACT_CODE_OID))
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report));
    let from_value = || match s.values.first() {
        Some(Cv::Coded(c)) => map_codeable_concept(c, ctx.terminology, ctx.report),
        Some(Cv::Text(t)) => t
            .value
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .map(|v| json!({ "text": v })),
        _ => None,
    };
    let mut d = from_code.or_else(from_value)?;
    if d["text"].is_null() {
        if let Some(t) = s.text.as_deref().filter(|t| !t.trim().is_empty()) {
            d["text"] = json!(t);
        }
    }
    Some(d)
}

/// The first value of an observation as a CodeableConcept (coded) or text.
fn value_concept(s: &Statement, ctx: &mut Context) -> Option<Value> {
    match s.values.first() {
        Some(Cv::Coded(c)) => map_codeable_concept(c, ctx.terminology, ctx.report),
        Some(Cv::Text(t)) => t
            .value
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .map(|v| json!({ "text": v })),
        _ => None,
    }
}

fn progress_status(s: &Statement, ctx: &mut Context) -> Option<Value> {
    value_concept(s, ctx).or_else(|| {
        s.text
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .map(|t| json!({ "text": t }))
    })
}

fn map_goal(s: &Statement, ctx: &mut Context) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Goal"));
    r.insert("id".into(), json!(ctx.next_id("goal")));
    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }

    let status = match s.status_code.as_deref() {
        Some(code) => lifecycle(code).unwrap_or_else(|| {
            ctx.report.warn(
                "goal.status.unmapped",
                "a statusCode is not a known goal status; lifecycleStatus was set to active",
            );
            "active"
        }),
        None => {
            ctx.report.warn(
                "goal.status.missing",
                "a goal has no statusCode; lifecycleStatus was set to active",
            );
            "active"
        }
    };
    r.insert("lifecycleStatus".into(), json!(status));

    match description(s, ctx) {
        Some(d) => {
            r.insert("description".into(), d);
        }
        None => {
            ctx.report.warn(
                "goal.description.missing",
                "a goal has no usable code or value; description could not be set",
            );
        }
    }
    r.insert("subject".into(), json!({ "reference": ctx.patient }));

    if let Some(t) = &s.effective_time {
        let low = t
            .value
            .as_deref()
            .or(t.low.as_ref().and_then(|p| p.value.as_deref()));
        if let Some(d) = low.and_then(|v| map_datetime(v, ctx.report)) {
            // `startDate` is a date; keep the date part of a datetime.
            r.insert("startDate".into(), json!(d.split('T').next().unwrap_or(&d)));
        }
        if let Some(d) = t
            .high
            .as_ref()
            .and_then(|p| p.value.as_deref())
            .and_then(|v| map_datetime(v, ctx.report))
        {
            r.insert(
                "target".into(),
                json!([{ "dueDate": d.split('T').next().unwrap_or(&d) }]),
            );
        }
    }

    // Progress Toward Goal: the latest coded or written progress, kept as the document states it.
    // The R4 achievement codes are not guessed from free text.
    if let Some(progress) = s
        .related
        .iter()
        .map(|x| &x.statement)
        .find(|x| x.has_template(PROGRESS_TOWARD_GOAL))
    {
        match progress_status(progress, ctx) {
            Some(v) => {
                r.insert("achievementStatus".into(), v);
            }
            None => ctx.report.info(
                "goal.progress.empty",
                "a progress observation has no usable value; achievementStatus was not set",
            ),
        }
    }

    // Priority Preference: how much the patient cares about the goal.
    if let Some(p) = s
        .related
        .iter()
        .map(|x| &x.statement)
        .find(|x| x.has_template(PRIORITY_PREFERENCE))
    {
        if let Some(v) = value_concept(p, ctx) {
            r.insert("priority".into(), v);
        }
    }

    let authors: Vec<_> = s.authors.iter().collect();
    apply_authors(&mut r, FIELDS, &authors, ctx);
    r.remove("_authored");
    Value::Object(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(code: &str, body: &str) -> (Vec<Value>, Report) {
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section>
  <code code="61146-7" codeSystem="2.16.840.1.113883.6.1"/>
  <entry><observation classCode="OBS" moodCode="INT">
    <templateId root="2.16.840.1.113883.10.20.22.4.121"/>
    <id root="1.2.3" extension="G-1"/>
    {code}
    {body}
  </observation></entry>
</section></component></structuredBody></component></ClinicalDocument>"#
        );
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        (map_goals(&d, &mut ctx), report)
    }

    const LOINC_GOAL: &str = r#"<code code="289169006" codeSystem="2.16.840.1.113883.6.96"/>"#;

    #[test]
    fn status_map() {
        for (code, want) in [
            ("active", "active"),
            ("completed", "completed"),
            ("aborted", "cancelled"),
            ("suspended", "on-hold"),
            ("new", "proposed"),
        ] {
            let (out, _) = run(LOINC_GOAL, &format!(r#"<statusCode code="{code}"/>"#));
            assert_eq!(out[0]["lifecycleStatus"], want, "{code}");
        }
        let (out, report) = run(LOINC_GOAL, "");
        assert_eq!(out[0]["lifecycleStatus"], "active");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "goal.status.missing"));
    }

    #[test]
    fn description_is_the_code_or_the_value_for_assertion() {
        let (out, _) = run(LOINC_GOAL, r#"<statusCode code="active"/>"#);
        assert_eq!(out[0]["description"]["coding"][0]["code"], "289169006");
        let assertion = r#"<code code="ASSERTION" codeSystem="2.16.840.1.113883.5.4"/>"#;
        let (out, _) = run(
            assertion,
            r#"<statusCode code="active"/><value xsi:type="ST">Walk daily</value>"#,
        );
        assert_eq!(out[0]["description"]["text"], "Walk daily");
        let (out, _) = run(
            assertion,
            r#"<statusCode code="active"/><value xsi:type="CD" code="1" codeSystem="2.16.840.1.113883.6.96"/>"#,
        );
        assert_eq!(out[0]["description"]["coding"][0]["code"], "1");
    }

    #[test]
    fn dates_become_start_and_target() {
        let (out, _) = run(
            LOINC_GOAL,
            r#"<statusCode code="active"/><effectiveTime><low value="20240101"/><high value="20241231"/></effectiveTime>"#,
        );
        assert_eq!(out[0]["startDate"], "2024-01-01");
        assert_eq!(out[0]["target"][0]["dueDate"], "2024-12-31");
    }

    #[test]
    fn missing_description_is_reported() {
        let (out, report) = run("", r#"<statusCode code="active"/>"#);
        assert!(out[0].get("description").is_none());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "goal.description.missing"));
    }

    #[test]
    fn progress_and_priority_are_mapped() {
        let body = r#"<statusCode code="active"/><effectiveTime><low value="20240101"/></effectiveTime>
<value xsi:type="CD" code="1" codeSystem="2.16.840.1.113883.6.96" displayName="x"/>
<entryRelationship typeCode="REFR"><observation classCode="OBS" moodCode="EVN"><templateId root="2.16.840.1.113883.10.20.22.4.110"/><code code="23" codeSystem="2.16.840.1.113883.6.1"/><value xsi:type="ST">Improving steadily</value></observation></entryRelationship>
<entryRelationship typeCode="REFR"><observation classCode="OBS" moodCode="EVN"><templateId root="2.16.840.1.113883.10.20.22.4.143"/><code code="225773000" codeSystem="2.16.840.1.113883.6.96"/><value xsi:type="CD" code="394849002" codeSystem="2.16.840.1.113883.6.96"/></observation></entryRelationship>"#;
        let (out, _) = run("active", body);
        assert_eq!(out[0]["achievementStatus"]["text"], "Improving steadily");
        assert_eq!(out[0]["priority"]["coding"][0]["code"], "394849002");
    }
}
