//! Plan of Treatment section to FHIR `CarePlan`.
//!
//! No IG page covers care plans; this follows C-CDA and the US Core CarePlan profile.
//! See `docs/decisions/0015-care-plan.md`.

use ccda_model::ClinicalDocument;
use serde_json::{json, Map, Value};

use crate::util::narrative_div;
use crate::Context;

const PLAN_OF_TREATMENT: &str = "2.16.840.1.113883.10.20.22.2.10";
const CATEGORY_SYSTEM: &str = "http://hl7.org/fhir/us/core/CodeSystem/careplan-category";

/// One `CarePlan` per Plan of Treatment section that has narrative or goals.
///
/// `goals` are the Goal resources already mapped; the plan points at all of them.
pub fn map_care_plans(doc: &ClinicalDocument, ctx: &mut Context, goals: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        if !section.has_template(PLAN_OF_TREATMENT) {
            continue;
        }
        let narrative = section
            .text
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty());
        let goal_refs: Vec<Value> = goals
            .iter()
            .filter_map(|g| g["id"].as_str())
            .map(|id| json!({ "reference": ctx.reference("Goal", id) }))
            .collect();
        if narrative.is_none() && goal_refs.is_empty() {
            continue;
        }

        let mut r = Map::new();
        r.insert("resourceType".into(), json!("CarePlan"));
        r.insert("id".into(), json!(ctx.next_id("careplan")));
        // The document does not say whether the plan is still in force; "active" is the
        // usual meaning of a current plan of treatment, and the guess is reported.
        ctx.report.info(
            "careplan.status.assumed",
            "a care plan status was not in the document; status was set to active",
        );
        r.insert("status".into(), json!("active"));
        r.insert("intent".into(), json!("plan"));
        r.insert(
            "category".into(),
            json!([{ "coding": [{ "system": CATEGORY_SYSTEM, "code": "assess-plan" }] }]),
        );
        if let Some(t) = section.title.as_deref().filter(|t| !t.trim().is_empty()) {
            r.insert("title".into(), json!(t));
        }
        // Narrative is required by US Core; it is the section text, flattened.
        let div = narrative_div(narrative.unwrap_or("See goals."));
        r.insert("text".into(), json!({ "status": "generated", "div": div }));
        r.insert("subject".into(), json!({ "reference": ctx.patient }));
        if !goal_refs.is_empty() {
            r.insert("goal".into(), json!(goal_refs));
        }
        out.push(Value::Object(r));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(section_body: &str, goals: &[Value]) -> (Vec<Value>, Report) {
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><component><structuredBody><component><section>
<templateId root="2.16.840.1.113883.10.20.22.2.10"/>
<code code="18776-5" codeSystem="2.16.840.1.113883.6.1"/>
<title>Plan of Treatment</title>{section_body}
</section></component></structuredBody></component></ClinicalDocument>"#
        );
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        (map_care_plans(&d, &mut ctx, goals), report)
    }

    #[test]
    fn plan_has_required_fields_and_escaped_narrative() {
        let (out, report) = run("<text>Follow up &lt;2 weeks &amp; recheck</text>", &[]);
        assert_eq!(out[0]["status"], "active");
        assert_eq!(out[0]["intent"], "plan");
        assert_eq!(out[0]["category"][0]["coding"][0]["code"], "assess-plan");
        let div = out[0]["text"]["div"].as_str().unwrap();
        assert!(div.contains("&lt;2 weeks &amp; recheck"), "{div}");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "careplan.status.assumed"));
    }

    #[test]
    fn plan_points_at_goals() {
        let goals = [json!({ "resourceType": "Goal", "id": "goal-1" })];
        let (out, _) = run("", &goals);
        assert!(out[0]["goal"][0]["reference"]
            .as_str()
            .unwrap()
            .starts_with("urn:uuid:"));
    }

    #[test]
    fn empty_section_gives_no_plan() {
        let (out, _) = run("", &[]);
        assert!(out.is_empty());
    }
}
