//! Policy Activity (template 4.61) to FHIR `Coverage`.
//!
//! No IG page covers insurance; this follows C-CDA (Payers section) and the US Core Coverage
//! profile. See `docs/decisions/0017-coverage.md`.

use ccda_model::{ClinicalDocument, Participant, Statement};
use serde_json::{json, Map, Value};

use crate::author::register_organization;
use crate::{map_codeable_concept, map_datetime, map_identifier, Context};

const POLICY_ACTIVITY: &str = "2.16.840.1.113883.10.20.22.4.61";
const COVERED_PARTY: &str = "2.16.840.1.113883.10.20.22.4.89";
const POLICY_HOLDER: &str = "2.16.840.1.113883.10.20.22.4.90";
const RELATIONSHIP_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/subscriber-relationship";
const COVERAGE_CLASS_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/coverage-class";
const V2_IDENTIFIER_TYPE: &str = "http://terminology.hl7.org/CodeSystem/v2-0203";

/// Map every Policy Activity in the document.
pub fn map_coverage(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut policies = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            collect(&entry.statement, &mut policies);
        }
    }
    policies.into_iter().map(|p| map_policy(p, ctx)).collect()
}

fn collect<'a>(s: &'a Statement, out: &mut Vec<&'a Statement>) {
    if s.has_template(POLICY_ACTIVITY) {
        out.push(s);
    }
    for r in &s.related {
        collect(&r.statement, out);
    }
}

fn participant<'a>(s: &'a Statement, template: &str, type_code: &str) -> Option<&'a Participant> {
    s.participants.iter().find(|p| {
        p.template_ids.iter().any(|t| t.root == template)
            || p.type_code.as_deref() == Some(type_code)
    })
}

/// FHIR subscriber-relationship code for a v3 RoleCode.
fn relationship(code: &str) -> &'static str {
    match code {
        "SELF" => "self",
        "SPS" | "SPOUSE" | "HUSB" | "WIFE" => "spouse",
        "CHILD" | "NCHILD" | "SON" | "DAU" | "STPCHLD" | "ADOPT" => "child",
        "PRN" | "MTH" | "FTH" | "STPPRN" | "NPRN" | "ADOPTP" => "parent",
        _ => "other",
    }
}

fn map_policy(s: &Statement, ctx: &mut Context) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Coverage"));
    r.insert("id".into(), json!(ctx.next_id("coverage")));

    // Coverage has no way to say "completed"; a policy in the document is taken as in force
    // unless it was nullified.
    let status = if s.status_code.as_deref() == Some("nullified") {
        "entered-in-error"
    } else {
        "active"
    };
    r.insert("status".into(), json!(status));
    r.insert("beneficiary".into(), json!({ "reference": ctx.patient }));

    if let Some(t) = s
        .code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
    {
        r.insert("type".into(), t);
    }

    // The covered party's id is the member id; the policy holder's is the subscriber id.
    let covered = participant(s, COVERED_PARTY, "COV");
    let holder = participant(s, POLICY_HOLDER, "HLD");
    let identifiers: Vec<Value> = covered
        .iter()
        .flat_map(|p| p.ids.iter())
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .map(|mut i| {
            i["type"] = json!({ "coding": [{ "system": V2_IDENTIFIER_TYPE, "code": "MB" }] });
            i
        })
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    let subscriber = holder
        .into_iter()
        .chain(covered)
        .flat_map(|p| p.ids.iter())
        .find_map(|id| id.extension.clone());
    if let Some(v) = subscriber {
        r.insert("subscriberId".into(), json!(v));
    }

    match covered
        .and_then(|p| p.role_code.as_ref())
        .and_then(|c| c.code.as_deref())
    {
        Some(code) => {
            let rel = relationship(code);
            if rel == "other" && code != "OTHER" {
                ctx.report.info(
                    "coverage.relationship.other",
                    "a covered party role code is not a known relationship; relationship was set to other",
                );
            }
            r.insert(
                "relationship".into(),
                json!({ "coding": [{ "system": RELATIONSHIP_SYSTEM, "code": rel }] }),
            );
        }
        None => {
            ctx.report.info(
                "coverage.relationship.missing",
                "a policy has no covered party role; relationship could not be set",
            );
        }
    }

    if let Some(t) = &s.effective_time {
        let low = t
            .low
            .as_ref()
            .and_then(|p| p.value.as_deref())
            .or(t.value.as_deref());
        let high = t.high.as_ref().and_then(|p| p.value.as_deref());
        let mut p = Map::new();
        if let Some(d) = low.and_then(|v| map_datetime(v, ctx.report)) {
            p.insert("start".into(), json!(d));
        }
        if let Some(d) = high.and_then(|v| map_datetime(v, ctx.report)) {
            p.insert("end".into(), json!(d));
        }
        if !p.is_empty() {
            r.insert("period".into(), Value::Object(p));
        }
    }

    // The policy's own ids are the group number.
    let groups: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|i| i.extension.as_deref())
        .map(|v| {
            json!({
                "type": { "coding": [{ "system": COVERAGE_CLASS_SYSTEM, "code": "group" }] },
                "value": v
            })
        })
        .collect();
    if !groups.is_empty() {
        r.insert("class".into(), json!(groups));
    }

    // Payer: the performer that is not a guarantor.
    let payor = s
        .performers
        .iter()
        .filter(|p| {
            !matches!(
                p.role_code.as_ref().and_then(|c| c.code.as_deref()),
                Some("GUAR" | "GUARD")
            )
        })
        .find_map(|p| {
            p.organization
                .as_ref()
                .and_then(|o| register_organization(o, ctx))
                .map(|reference| json!({ "reference": reference }))
        });
    match payor {
        Some(p) => {
            r.insert("payor".into(), json!([p]));
        }
        None => {
            ctx.report.warn(
                "coverage.payor.missing",
                "a policy has no payer organization with a name; payor could not be set",
            );
        }
    }
    Value::Object(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(body: &str) -> (Vec<Value>, Report) {
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><component><structuredBody><component><section>
<entry><act classCode="ACT" moodCode="EVN">
 <templateId root="2.16.840.1.113883.10.20.22.4.60"/>
 <entryRelationship typeCode="COMP"><act classCode="ACT" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.61"/>
  <id root="1.2.3" extension="GRP-1"/>
  <code code="1" codeSystem="2.16.840.1.113883.3.221.5"/>
  <statusCode code="completed"/>
  {body}
 </act></entryRelationship>
</act></entry>
</section></component></structuredBody></component></ClinicalDocument>"#
        );
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        (map_coverage(&d, &mut ctx), report)
    }

    const FULL: &str = r#"
  <performer typeCode="PRF"><templateId root="2.16.840.1.113883.10.20.22.4.87"/>
    <assignedEntity><id root="2.16.840.1.113883.6.300" extension="PAYER-1"/>
      <representedOrganization><name>Example Health Plan</name></representedOrganization>
    </assignedEntity></performer>
  <participant typeCode="COV"><time><low value="20240101"/><high value="20241231"/></time>
    <participantRole><templateId root="2.16.840.1.113883.10.20.22.4.89"/>
      <id root="1.2.3.4" extension="MEMBER-9"/>
      <code code="SELF" codeSystem="2.16.840.1.113883.5.111"/></participantRole></participant>
  <participant typeCode="HLD">
    <participantRole><templateId root="2.16.840.1.113883.10.20.22.4.90"/>
      <id root="1.2.3.4" extension="SUB-7"/></participantRole></participant>"#;

    #[test]
    fn policy_becomes_coverage() {
        let (out, _) = run(FULL);
        let c = &out[0];
        assert_eq!(c["status"], "active");
        assert_eq!(c["subscriberId"], "SUB-7");
        assert_eq!(c["identifier"][0]["value"], "MEMBER-9");
        assert_eq!(c["relationship"]["coding"][0]["code"], "self");
        assert_eq!(c["class"][0]["value"], "GRP-1");
        assert!(c["payor"][0]["reference"]
            .as_str()
            .unwrap()
            .starts_with("urn:uuid:"));
        assert_eq!(c["type"]["coding"][0]["system"], "https://nahdo.org/sopt");
    }

    #[test]
    fn missing_payer_and_role_are_reported() {
        let (out, report) = run("");
        assert!(out[0].get("payor").is_none());
        for code in ["coverage.payor.missing", "coverage.relationship.missing"] {
            assert!(report.issues.iter().any(|i| i.code == code), "{code}");
        }
    }

    #[test]
    fn relationship_codes() {
        assert_eq!(relationship("SPS"), "spouse");
        assert_eq!(relationship("NCHILD"), "child");
        assert_eq!(relationship("XYZ"), "other");
    }
}
