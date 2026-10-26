//! Care Team Organizer (template 4.500) to FHIR `CareTeam`.
//!
//! No IG page covers care teams; this follows C-CDA and the US Core CareTeam profile.
//! See `docs/decisions/0016-care-team.md`.

use ccda_model::{ClinicalDocument, Statement, StatementKind};
use serde_json::{json, Map, Value};

use crate::author::{register_organization, register_practitioner};
use crate::{map_codeable_concept, map_datetime, map_identifier, Context};

const CARE_TEAM_ORGANIZER: &str = "2.16.840.1.113883.10.20.22.4.500";

fn status(code: &str) -> Option<&'static str> {
    Some(match code {
        "active" => "active",
        "completed" => "inactive",
        "suspended" | "held" => "suspended",
        "aborted" | "cancelled" => "inactive",
        "nullified" => "entered-in-error",
        "new" => "proposed",
        _ => return None,
    })
}

fn period(raw_low: Option<&str>, raw_high: Option<&str>, ctx: &mut Context) -> Option<Value> {
    let mut p = Map::new();
    if let Some(d) = raw_low.and_then(|v| map_datetime(v, ctx.report)) {
        p.insert("start".into(), json!(d));
    }
    if let Some(d) = raw_high.and_then(|v| map_datetime(v, ctx.report)) {
        p.insert("end".into(), json!(d));
    }
    (!p.is_empty()).then_some(Value::Object(p))
}

/// Map every Care Team Organizer in the document.
pub fn map_care_teams(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind == StatementKind::Organizer && s.has_template(CARE_TEAM_ORGANIZER) {
                out.push(map_team(s, ctx));
            }
        }
    }
    out
}

fn map_team(s: &Statement, ctx: &mut Context) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("CareTeam"));
    r.insert("id".into(), json!(ctx.next_id("careteam")));
    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    match s.status_code.as_deref().and_then(status) {
        Some(st) => {
            r.insert("status".into(), json!(st));
        }
        None => {
            ctx.report.warn(
                "careteam.status.assumed",
                "a care team has no usable statusCode; status was set to active",
            );
            r.insert("status".into(), json!("active"));
        }
    }
    r.insert("subject".into(), json!({ "reference": ctx.patient }));
    if let Some(t) = &s.effective_time {
        let low = t
            .low
            .as_ref()
            .and_then(|p| p.value.as_deref())
            .or(t.value.as_deref());
        let high = t.high.as_ref().and_then(|p| p.value.as_deref());
        if let Some(p) = period(low, high, ctx) {
            r.insert("period".into(), p);
        }
    }

    let mut participants = Vec::new();
    for related in &s.related {
        let member = &related.statement;
        for performer in &member.performers {
            let Some(reference) = register_practitioner(performer, ctx) else {
                ctx.report.warn(
                    "careteam.member.unidentified",
                    "a care team member has no name or id and was skipped",
                );
                continue;
            };
            let mut p = Map::new();
            // Roles: what the member does on the team (functionCode on the performer or the act),
            // then the performer's own role or specialty. Repeats are dropped.
            let mut roles: Vec<Value> = Vec::new();
            for code in [
                performer.function_code.as_ref(),
                member.function_code.as_ref(),
                performer.role_code.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                if let Some(role) = map_codeable_concept(code, ctx.terminology, ctx.report) {
                    if !roles.contains(&role) {
                        roles.push(role);
                    }
                }
            }
            if !roles.is_empty() {
                p.insert("role".into(), json!(roles));
            }
            p.insert("member".into(), json!({ "reference": reference }));
            if let Some(org) = performer
                .organization
                .as_ref()
                .and_then(|o| register_organization(o, ctx))
            {
                p.insert("onBehalfOf".into(), json!({ "reference": org }));
            }
            let t = member.effective_time.as_ref();
            let low = t
                .and_then(|t| t.low.as_ref())
                .and_then(|p| p.value.as_deref())
                .or_else(|| t.and_then(|t| t.value.as_deref()));
            let high = t
                .and_then(|t| t.high.as_ref())
                .and_then(|p| p.value.as_deref());
            if let Some(per) = period(low, high, ctx) {
                p.insert("period".into(), per);
            }
            participants.push(Value::Object(p));
        }
    }
    if participants.is_empty() {
        ctx.report.warn(
            "careteam.members.missing",
            "a care team has no members that could be mapped",
        );
    } else {
        r.insert("participant".into(), json!(participants));
    }
    Value::Object(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(members: &str, status: &str) -> (Vec<Value>, Report) {
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><component><structuredBody><component><section>
<code code="85847-2" codeSystem="2.16.840.1.113883.6.1"/>
<entry><organizer classCode="CLUSTER" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.500"/>
  <id root="1.2.3" extension="T-1"/>
  <statusCode code="{status}"/>
  <effectiveTime><low value="20240101"/></effectiveTime>
  {members}
</organizer></entry>
</section></component></structuredBody></component></ClinicalDocument>"#
        );
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        (map_care_teams(&d, &mut ctx), report)
    }

    const MEMBER: &str = r#"<component><act classCode="PCPR" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.500.1"/>
  <effectiveTime><low value="20240201"/></effectiveTime>
  <performer><assignedEntity>
    <id root="2.16.840.1.113883.4.6" extension="1234567893"/>
    <code code="163W00000X" codeSystem="2.16.840.1.113883.6.101"/>
    <assignedPerson><name><given>Ann</given><family>Doe</family></name></assignedPerson>
  </assignedEntity></performer>
</act></component>"#;

    #[test]
    fn member_becomes_participant_with_role_and_period() {
        let (out, _) = run(MEMBER, "active");
        assert_eq!(out[0]["status"], "active");
        assert_eq!(out[0]["period"]["start"], "2024-01-01");
        let p = &out[0]["participant"][0];
        assert_eq!(p["role"][0]["coding"][0]["code"], "163W00000X");
        assert_eq!(p["period"]["start"], "2024-02-01");
        assert!(p["member"]["reference"]
            .as_str()
            .unwrap()
            .starts_with("urn:uuid:"));
    }

    #[test]
    fn status_map_and_fallback() {
        assert_eq!(run(MEMBER, "completed").0[0]["status"], "inactive");
        let (out, report) = run(MEMBER, "bogus");
        assert_eq!(out[0]["status"], "active");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "careteam.status.assumed"));
    }

    #[test]
    fn team_without_members_is_reported() {
        let (out, report) = run("", "active");
        assert!(out[0].get("participant").is_none());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "careteam.members.missing"));
    }

    #[test]
    fn function_code_becomes_a_role_before_the_specialty() {
        let member = MEMBER.replace(
            "<performer>",
            r#"<functionCode code="106292003" codeSystem="2.16.840.1.113883.6.96"/><performer><functionCode code="PCP" codeSystem="2.16.840.1.113883.5.88"/>"#,
        );
        let (out, _) = run(&member, "active");
        let roles = out[0]["participant"][0]["role"].as_array().unwrap();
        assert_eq!(roles.len(), 3);
        assert_eq!(roles[0]["coding"][0]["code"], "PCP");
        assert_eq!(roles[1]["coding"][0]["code"], "106292003");
        assert_eq!(roles[2]["coding"][0]["code"], "163W00000X");
    }
}
