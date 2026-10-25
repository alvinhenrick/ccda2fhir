//! Goal Observation (template 4.121) to FHIR `Goal`.
//!
//! No IG page covers goals; this follows C-CDA, the US Core Goal profile and common practice.
//! See `docs/decisions/0014-goals.md`.

use ccda_model::{ClinicalDocument, Statement, StatementKind, Value as Cv};
use serde_json::{json, Map, Value};

use crate::author::{apply_authors, AuthorFields};
use crate::{map_codeable_concept, map_datetime, map_identifier, Context};

const GOAL_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.121";
const PROGRESS_TOWARD_GOAL: &str = "2.16.840.1.113883.10.20.22.4.110";
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

    if s.related
        .iter()
        .any(|x| x.statement.has_template(PROGRESS_TOWARD_GOAL))
    {
        ctx.report.info(
            "goal.progress.unsupported",
            "progress toward a goal was not mapped yet (achievementStatus)",
        );
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
}
