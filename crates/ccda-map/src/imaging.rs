//! DICOM Object Catalog (Study Act, Series Act, SOP Instance Observation) to FHIR `ImagingStudy`.
//!
//! The templates are `2.16.840.1.113883.10.20.6.2.6` (Study), `...22.4.63` (Series) and
//! `...6.2.8` (SOP Instance). Their constraints come from the C-CDA schematron. UIDs are
//! written the way FHIR writes DICOM UIDs: `urn:oid:` values.

use ccda_model::{ClinicalDocument, Statement, StatementKind};
use serde_json::{json, Map, Value};

use crate::nullflavor::data_absent_reason_extension;
use crate::{map_datetime, Context};

const STUDY_ACT: &str = "2.16.840.1.113883.10.20.6.2.6";
const SERIES_ACT: &str = "2.16.840.1.113883.10.20.22.4.63";
const SOP_INSTANCE: &str = "2.16.840.1.113883.10.20.6.2.8";
const DICOM_UID_SYSTEM: &str = "urn:dicom:uid";
const DCM_SYSTEM: &str = "http://dicom.nema.org/resources/ontology/DCM";
const URI_SYSTEM: &str = "urn:ietf:rfc:3986";
/// The qualifier name that carries a series modality (DCM 121139).
const MODALITY_QUALIFIER: &str = "121139";

fn uid(s: &Statement) -> Option<String> {
    s.ids
        .iter()
        .find_map(|i| i.root.as_deref())
        .map(|r| format!("urn:oid:{r}"))
}

fn instance(s: &Statement, number: usize) -> Option<Value> {
    let mut i = Map::new();
    i.insert("uid".into(), json!(uid(s)?.trim_start_matches("urn:oid:")));
    // The SOP class is the code of the observation (a DICOM UID).
    if let Some(class) = s.code.as_ref().and_then(|c| c.code.as_deref()) {
        i.insert(
            "sopClass".into(),
            json!({ "system": URI_SYSTEM, "code": format!("urn:oid:{class}") }),
        );
    } else {
        i.insert(
            "sopClass".into(),
            json!({ "extension": [{ "url": "http://hl7.org/fhir/StructureDefinition/data-absent-reason", "valueCode": "unknown" }] }),
        );
    }
    i.insert("number".into(), json!(number));
    Some(Value::Object(i))
}

fn series(s: &Statement, number: usize, ctx: &mut Context) -> Option<Value> {
    let mut r = Map::new();
    r.insert("uid".into(), json!(uid(s)?.trim_start_matches("urn:oid:")));
    r.insert("number".into(), json!(number));
    let modality = s.code.as_ref().and_then(|c| {
        c.qualifiers
            .iter()
            .find(|(n, _)| n.code.as_deref() == Some(MODALITY_QUALIFIER))
            .and_then(|(_, v)| v.code.clone())
    });
    r.insert(
        "modality".into(),
        match modality {
            Some(code) => json!({ "system": DCM_SYSTEM, "code": code }),
            None => {
                ctx.report.warn(
                    "imaging.modality.missing",
                    "a series has no modality qualifier; modality was marked absent",
                );
                json!({ "extension": [data_absent_reason_extension("UNK", ctx.report)] })
            }
        },
    );
    let instances: Vec<Value> = s
        .related
        .iter()
        .map(|r| &r.statement)
        .filter(|i| i.kind == StatementKind::Observation && i.has_template(SOP_INSTANCE))
        .enumerate()
        .filter_map(|(n, i)| instance(i, n + 1))
        .collect();
    if !instances.is_empty() {
        r.insert("numberOfInstances".into(), json!(instances.len()));
        r.insert("instance".into(), json!(instances));
    }
    Some(Value::Object(r))
}

fn study(s: &Statement, ctx: &mut Context) -> Option<Value> {
    let study_uid = uid(s)?;
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("ImagingStudy"));
    r.insert("id".into(), json!(ctx.next_id("imagingstudy")));
    r.insert(
        "identifier".into(),
        json!([{ "system": DICOM_UID_SYSTEM, "value": study_uid }]),
    );
    r.insert("status".into(), json!("available"));
    r.insert("subject".into(), json!({ "reference": ctx.patient }));
    if let Some(started) = s
        .effective_time
        .as_ref()
        .and_then(|t| {
            t.value
                .as_deref()
                .or(t.low.as_ref().and_then(|l| l.value.as_deref()))
        })
        .and_then(|v| map_datetime(v, ctx.report))
    {
        r.insert("started".into(), json!(started));
    }
    let series_list: Vec<Value> = s
        .related
        .iter()
        .map(|r| &r.statement)
        .filter(|x| x.kind == StatementKind::Act && x.has_template(SERIES_ACT))
        .enumerate()
        .filter_map(|(n, x)| series(x, n + 1, ctx))
        .collect();
    if series_list.is_empty() {
        ctx.report.warn(
            "imaging.study.no-series",
            "a DICOM study has no series; the study was kept without series",
        );
    } else {
        let instances: usize = series_list
            .iter()
            .map(|x| x["instance"].as_array().map_or(0, Vec::len))
            .sum();
        r.insert("numberOfSeries".into(), json!(series_list.len()));
        r.insert("numberOfInstances".into(), json!(instances));
        r.insert("series".into(), json!(series_list));
    }
    Some(Value::Object(r))
}

/// Map every Study Act in the document.
pub fn map_imaging(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind == StatementKind::Act && s.has_template(STUDY_ACT) {
                match study(s, ctx) {
                    Some(r) => out.push(r),
                    None => ctx.report.warn(
                        "imaging.study.no-uid",
                        "a DICOM study has no id; it was not mapped",
                    ),
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    const DOC: &str = r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><component><structuredBody><component><section><entry>
<act classCode="ACT" moodCode="EVN"><templateId root="2.16.840.1.113883.10.20.6.2.6"/>
<id root="1.2.840.113619.2.55.3.1"/><code code="113014" codeSystem="1.2.840.10008.2.16.4"/>
<effectiveTime value="20240301101500-0500"/>
<entryRelationship typeCode="COMP"><act classCode="ACT" moodCode="EVN"><templateId root="2.16.840.1.113883.10.20.22.4.63"/>
<id root="1.2.840.113619.2.55.3.1.1"/>
<code code="113015" codeSystem="1.2.840.10008.2.16.4"><qualifier><name code="121139" codeSystem="1.2.840.10008.2.16.4"/><value code="CT" codeSystem="1.2.840.10008.2.16.4"/></qualifier></code>
<entryRelationship typeCode="COMP"><observation classCode="DGIMG" moodCode="EVN"><templateId root="2.16.840.1.113883.10.20.6.2.8"/>
<id root="1.2.840.113619.2.55.3.1.1.1"/><code code="1.2.840.10008.5.1.4.1.1.2" codeSystem="1.2.840.10008.2.6.1"/></observation></entryRelationship>
</act></entryRelationship></act></entry></section></component></structuredBody></component></ClinicalDocument>"#;

    #[test]
    fn study_series_and_instance_are_mapped() {
        let doc = ccda_model::parse(DOC.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_imaging(&doc, &mut ctx);
        assert_eq!(out.len(), 1);
        let s = &out[0];
        assert_eq!(
            s["identifier"][0]["value"],
            "urn:oid:1.2.840.113619.2.55.3.1"
        );
        assert_eq!(s["series"][0]["modality"]["code"], "CT");
        assert_eq!(
            s["series"][0]["instance"][0]["sopClass"]["code"],
            "urn:oid:1.2.840.10008.5.1.4.1.1.2"
        );
        assert_eq!(s["numberOfInstances"], 1);
        assert_eq!(s["started"], "2024-03-01T10:15:00-05:00");
    }

    #[test]
    fn missing_modality_is_marked_absent_and_reported() {
        let xml = DOC.replace(
            r#"<qualifier><name code="121139" codeSystem="1.2.840.10008.2.16.4"/><value code="CT" codeSystem="1.2.840.10008.2.16.4"/></qualifier>"#,
            "",
        );
        let doc = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_imaging(&doc, &mut ctx);
        assert!(out[0]["series"][0]["modality"]["extension"].is_array());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "imaging.modality.missing"));
    }
}
