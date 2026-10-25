//! Advance Directive Observation (4.48) and Organizer (4.108) to FHIR `Consent`.
//!
//! The observation says which directive exists (`code`, for example resuscitation status) and
//! its value (for example "do not resuscitate"). `Consent` with scope `adr` is the R4 resource
//! for an advance care directive. The value goes in `provision.code`; the provision has no
//! `type`, because the document does not say permit or deny. See ADR 0044.

use ccda_model::{ClinicalDocument, Statement, StatementKind, Value as Cv};
use serde_json::{json, Map, Value};

use crate::author::{apply_authors, AuthorFields};
use crate::util::nested;
use crate::{
    data_absent_reason_extension, map_codeable_concept, map_datetime, map_identifier, Context,
};

const OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.48";
const ORGANIZER: &str = "2.16.840.1.113883.10.20.22.4.108";
const SCOPE_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/consentscope";

const FIELDS: AuthorFields = AuthorFields {
    recorder: None,
    date: "dateTime",
};

/// Map every Advance Directive Observation, alone or inside an organizer.
pub fn map_advance_directives(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.has_template(OBSERVATION) {
                out.push(map_directive(s, ctx));
            } else if s.has_template(ORGANIZER) {
                for inner in nested(s).filter(|i| i.has_template(OBSERVATION)) {
                    out.push(map_directive(inner, ctx));
                }
            }
        }
    }
    out
}

fn status_of(code: Option<&str>) -> Option<&'static str> {
    Some(match code? {
        "completed" | "active" => "active",
        "aborted" | "cancelled" | "suspended" => "inactive",
        "nullified" => "entered-in-error",
        _ => return None,
    })
}

fn map_directive(s: &Statement, ctx: &mut Context) -> Value {
    debug_assert!(s.kind == StatementKind::Observation);
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Consent"));
    r.insert("id".into(), json!(ctx.next_id("consent")));
    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    let status = status_of(s.status_code.as_deref()).unwrap_or_else(|| {
        ctx.report.warn(
            "consent.status.assumed",
            "an advance directive has no usable statusCode; status was set to active",
        );
        "active"
    });
    r.insert("status".into(), json!(status));
    r.insert(
        "scope".into(),
        json!({ "coding": [{ "system": SCOPE_SYSTEM, "code": "adr" }] }),
    );
    let category = s
        .code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report));
    r.insert(
        "category".into(),
        json!([category.unwrap_or_else(|| {
            ctx.report.warn(
                "consent.category.missing",
                "an advance directive has no type code; the category is the generic advance directive code",
            );
            json!({ "coding": [{ "system": "http://loinc.org", "code": "42348-3" }] })
        })]),
    );
    r.insert("patient".into(), json!({ "reference": ctx.patient }));
    // R4 requires a policy or a policyRule (ppc-1). C-CDA names none, so it is marked unknown.
    r.insert(
        "policyRule".into(),
        json!({ "extension": [data_absent_reason_extension("UNK", ctx.report)] }),
    );

    let authors: Vec<_> = s.authors.iter().collect();
    apply_authors(&mut r, FIELDS, &authors, ctx);

    let mut provision = Map::new();
    if let Some(t) = &s.effective_time {
        if let Some(low) = t
            .low
            .as_ref()
            .and_then(|l| l.value.as_deref())
            .or(t.value.as_deref())
            .and_then(|v| map_datetime(v, ctx.report))
        {
            r.insert("dateTime".into(), json!(low));
        }
        let mut period = Map::new();
        if let Some(v) = t
            .low
            .as_ref()
            .and_then(|l| l.value.as_deref())
            .and_then(|v| map_datetime(v, ctx.report))
        {
            period.insert("start".into(), json!(v));
        }
        if let Some(v) = t
            .high
            .as_ref()
            .and_then(|h| h.value.as_deref())
            .and_then(|v| map_datetime(v, ctx.report))
        {
            period.insert("end".into(), json!(v));
        }
        if !period.is_empty() {
            provision.insert("period".into(), Value::Object(period));
        }
    }
    if let Some(Cv::Coded(code)) = s.values.first() {
        if let Some(c) = map_codeable_concept(code, ctx.terminology, ctx.report) {
            provision.insert("code".into(), json!([c]));
        }
    }
    if !provision.is_empty() {
        r.insert("provision".into(), Value::Object(provision));
    }
    if !s.participants.is_empty() {
        ctx.report.info(
            "consent.participants.unsupported",
            "verifier, witness and custodian participants of an advance directive were not mapped",
        );
    }
    Value::Object(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    const DOC: &str = r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section><entry><observation classCode="OBS" moodCode="EVN">
<templateId root="2.16.840.1.113883.10.20.22.4.48"/>
<id root="1.2.3" extension="AD-1"/>
<code code="304251008" codeSystem="2.16.840.1.113883.6.96"/>
<statusCode code="completed"/>
<effectiveTime><low value="20240101"/><high value="20261231"/></effectiveTime>
<value xsi:type="CD" code="304253006" codeSystem="2.16.840.1.113883.6.96"/>
<participant typeCode="VRF"><participantRole/></participant>
</observation></entry></section></component></structuredBody></component></ClinicalDocument>"#;

    fn run(xml: &str) -> (Vec<Value>, Report) {
        let doc = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_advance_directives(&doc, &mut ctx);
        (out, report)
    }

    #[test]
    fn directive_becomes_an_adr_consent() {
        let (out, report) = run(DOC);
        let c = &out[0];
        assert_eq!(c["resourceType"], "Consent");
        assert_eq!(c["status"], "active");
        assert_eq!(c["scope"]["coding"][0]["code"], "adr");
        assert_eq!(c["category"][0]["coding"][0]["code"], "304251008");
        assert_eq!(c["provision"]["code"][0]["coding"][0]["code"], "304253006");
        assert_eq!(c["provision"]["period"]["end"], "2026-12-31");
        assert_eq!(c["dateTime"], "2024-01-01");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "consent.participants.unsupported"));
    }

    #[test]
    fn missing_type_is_reported() {
        let xml = DOC.replace(
            r#"<code code="304251008" codeSystem="2.16.840.1.113883.6.96"/>"#,
            "",
        );
        let (out, report) = run(&xml);
        assert_eq!(out[0]["category"][0]["coding"][0]["code"], "42348-3");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "consent.category.missing"));
    }
}
