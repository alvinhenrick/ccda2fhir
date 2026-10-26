//! Policy Activity (template 4.61) to FHIR `Coverage`.
//!
//! No IG page covers insurance; this follows C-CDA (Payers section) and the US Core Coverage
//! profile. See `docs/decisions/0017-coverage.md`.

use ccda_model::{ClinicalDocument, Participant, Statement};
use serde_json::{json, Map, Value};

use crate::author::register_organization;
use crate::{map_codeable_concept, map_datetime, map_identifier, Context};

const POLICY_ACTIVITY: &str = "2.16.840.1.113883.10.20.22.4.61";
const AUTHORIZATION_ACTIVITY: &str = "2.16.840.1.113883.10.20.1.19";
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

/// The policy holder as a `RelatedPerson`, when the document gives a name.
/// `relationship` is the person's relationship to the patient, when the document gives it.
pub(crate) fn related_person(
    p: &Participant,
    relationship: Option<Value>,
    ctx: &mut Context,
) -> Option<String> {
    let name = p
        .person_name
        .as_ref()
        .and_then(crate::patient::human_name)?;
    let id = ctx.next_id("relatedperson");
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("RelatedPerson"));
    r.insert("id".into(), json!(id));
    r.insert("active".into(), json!(true));
    r.insert("patient".into(), json!({ "reference": ctx.patient }));
    if let Some(rel) = relationship {
        r.insert("relationship".into(), json!([rel]));
    }
    r.insert("name".into(), json!([name]));
    let identifiers: Vec<Value> = p
        .ids
        .iter()
        .filter_map(|i| map_identifier(i, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    let telecoms: Vec<Value> = p
        .telecoms
        .iter()
        .filter_map(crate::patient::telecom)
        .collect();
    if !telecoms.is_empty() {
        r.insert("telecom".into(), json!(telecoms));
    }
    let addresses: Vec<Value> = p
        .addresses
        .iter()
        .filter_map(crate::patient::address)
        .collect();
    if !addresses.is_empty() {
        r.insert("address".into(), json!(addresses));
    }
    let reference = ctx.reference("RelatedPerson", &id);
    ctx.push_extra(Value::Object(r));
    Some(reference)
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

    // The subscriber: the patient when the covered party is the policy holder, otherwise the
    // policy holder as a RelatedPerson when the document names them.
    let is_self = covered
        .and_then(|p| p.role_code.as_ref())
        .and_then(|c| c.code.as_deref())
        == Some("SELF");
    if is_self {
        r.insert("subscriber".into(), json!({ "reference": ctx.patient }));
    } else if let Some(reference) = holder.and_then(|h| related_person(h, None, ctx)) {
        r.insert("subscriber".into(), json!({ "reference": reference }));
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

    // The policy's own interval, else the covered party's `time`.
    let (member_low, member_high) = covered
        .map(|p| (p.time.as_deref(), p.time_high.as_deref()))
        .unwrap_or_default();
    let own = s.effective_time.as_ref().map(|t| {
        (
            t.low
                .as_ref()
                .and_then(|p| p.value.as_deref())
                .or(t.value.as_deref()),
            t.high.as_ref().and_then(|p| p.value.as_deref()),
        )
    });
    let span = match own {
        Some((None, None)) | None => (member_low, member_high),
        Some(own) => own,
    };
    {
        let (low, high) = span;
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

    // The policy's own ids are the group number; the plan entry gives the plan.
    let mut classes: Vec<Value> = s
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
    for plan in s
        .related_by("REFR")
        .filter(|p| p.mood_code.as_deref() == Some("DEF"))
    {
        let name = plan
            .text
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty());
        let value = plan
            .ids
            .iter()
            .find_map(|i| i.extension.as_deref())
            .or(name);
        if let Some(value) = value {
            let mut c = Map::new();
            c.insert(
                "type".into(),
                json!({ "coding": [{ "system": COVERAGE_CLASS_SYSTEM, "code": "plan" }] }),
            );
            c.insert("value".into(), json!(value));
            if let Some(n) = name {
                c.insert("name".into(), json!(n));
            }
            classes.push(Value::Object(c));
        }
    }
    if !classes.is_empty() {
        r.insert("class".into(), json!(classes));
    }

    if s.related
        .iter()
        .any(|x| x.statement.has_template(AUTHORIZATION_ACTIVITY))
    {
        ctx.report.info(
            "coverage.authorization.unsupported",
            "an authorization on a policy was not mapped; the IG has no mapping for it",
        );
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

    #[test]
    fn self_subscriber_is_the_patient() {
        let (out, _) = run(FULL);
        assert_eq!(out[0]["subscriber"], out[0]["beneficiary"]);
    }

    #[test]
    fn named_policy_holder_becomes_a_related_person() {
        let body = r#"
  <participant typeCode="COV"><participantRole><templateId root="2.16.840.1.113883.10.20.22.4.89"/>
    <id root="1.2.3.4" extension="MEMBER-9"/><code code="CHILD" codeSystem="2.16.840.1.113883.5.111"/></participantRole></participant>
  <participant typeCode="HLD"><participantRole><templateId root="2.16.840.1.113883.10.20.22.4.90"/>
    <id root="1.2.3.4" extension="SUB-7"/>
    <playingEntity><name><given>Pat</given><family>Holder</family></name></playingEntity></participantRole></participant>"#;
        let xml_doc = run(body);
        let c = &xml_doc.0[0];
        assert!(c["subscriber"]["reference"].is_string());
        assert_ne!(c["subscriber"], c["beneficiary"]);
    }

    #[test]
    fn authorization_is_reported_not_mapped() {
        let auth = r#"<entryRelationship typeCode="REFR"><act classCode="ACT" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.1.19"/><id root="1.2.3" extension="AUTH-1"/>
  <code code="AUTH" codeSystem="2.16.840.1.113883.5.4"/>
</act></entryRelationship>"#;
        let (_, report) = run(&format!("{FULL}{auth}"));
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "coverage.authorization.unsupported"));
    }

    #[test]
    fn plan_name_and_member_period() {
        let body = r#"
  <participant typeCode="COV"><time><low value="20240201"/><high value="20241231"/></time>
    <participantRole><templateId root="2.16.840.1.113883.10.20.22.4.89"/>
      <id root="1.2.3.4" extension="MEMBER-9"/><code code="SELF" codeSystem="2.16.840.1.113883.5.111"/></participantRole></participant>
  <entryRelationship typeCode="REFR"><act classCode="ACT" moodCode="DEF">
    <id root="1.2.3" extension="PLAN-5"/><text>Example Gold PPO</text></act></entryRelationship>"#;
        let (out, _) = run(body);
        let c = &out[0];
        assert_eq!(c["period"]["start"], "2024-02-01");
        assert_eq!(c["period"]["end"], "2024-12-31");
        let plan = c["class"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["type"]["coding"][0]["code"] == "plan")
            .unwrap();
        assert_eq!(plan["value"], "PLAN-5");
        assert_eq!(plan["name"], "Example Gold PPO");
    }
}
