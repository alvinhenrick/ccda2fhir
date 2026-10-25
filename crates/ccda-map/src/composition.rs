//! The document header and section structure to FHIR `Composition`.
//!
//! There is no IG page for this (the C-CDA on FHIR document profiles are outdated); this follows
//! FHIR R4 Composition. See `docs/decisions/0020-composition.md`.

use std::collections::{BTreeMap, BTreeSet};

use ccda_model::{ClinicalDocument, Section, Statement};
use serde_json::{json, Map, Value};

use crate::author::{register_author_agent, register_organization};
use crate::util::narrative_div;
use crate::{map_codeable_concept, map_datetime, map_identifier, Context, Report};

const CONFIDENTIALITY: [&str; 6] = ["U", "L", "M", "N", "R", "V"];
const EMPTY_REASON_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/list-empty-reason";

/// Resource types that are clinical content a section can list.
fn is_content(resource_type: &str) -> bool {
    !matches!(
        resource_type,
        "Patient"
            | "Practitioner"
            | "Organization"
            | "Provenance"
            | "Location"
            | "Specimen"
            | "Medication"
            | "Composition"
    )
}

/// `system|value` keys for a FHIR resource's identifiers.
fn keys(identifiers: &Value) -> Vec<String> {
    identifiers
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| {
            let value = i["value"].as_str()?;
            Some(format!(
                "{}|{value}",
                i["system"].as_str().unwrap_or_default()
            ))
        })
        .collect()
}

/// Build the Composition for `doc`. `resources` are the clinical resources already mapped; each
/// section lists the ones whose identifier matches an id of a statement in that section.
///
/// Returns `None` (reported) when a required element (author, date) is missing.
pub fn map_composition(
    doc: &ClinicalDocument,
    ctx: &mut Context,
    resources: &[Value],
) -> Option<Value> {
    let authors: Vec<String> = doc
        .authors
        .iter()
        .filter_map(|a| register_author_agent(a, ctx))
        .collect();
    if authors.is_empty() {
        ctx.report.warn(
            "composition.author.missing",
            "the document has no author that is a person, software or organization; no Composition was created (author is required)",
        );
        return None;
    }
    let Some(date) = doc
        .effective_time
        .as_deref()
        .and_then(|t| map_datetime(t, ctx.report))
    else {
        ctx.report.warn(
            "composition.date.missing",
            "the document has no usable effectiveTime; no Composition was created (date is required)",
        );
        return None;
    };

    // Which resources carry which identifier.
    let mut by_key: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in resources {
        let (Some(t), Some(id)) = (r["resourceType"].as_str(), r["id"].as_str()) else {
            continue;
        };
        if !is_content(t) {
            continue;
        }
        let reference = ctx.reference(t, id);
        for k in keys(&r["identifier"]) {
            by_key.entry(k).or_default().push(reference.clone());
        }
    }

    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Composition"));
    r.insert("id".into(), json!(ctx.next_id("composition")));
    if let Some(i) = doc
        .id
        .as_ref()
        .and_then(|i| map_identifier(i, ctx.terminology, ctx.report))
    {
        r.insert("identifier".into(), i);
    }
    // The document does not say whether it is final; a document that has been issued is.
    ctx.report.info(
        "composition.status.assumed",
        "a Composition status is not in the document; status was set to final",
    );
    r.insert("status".into(), json!("final"));
    let type_ = doc
        .code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report));
    match &type_ {
        Some(t) => {
            r.insert("type".into(), t.clone());
        }
        None => {
            ctx.report.warn(
                "composition.type.missing",
                "the document has no usable code; Composition.type could not be set",
            );
        }
    }
    r.insert("subject".into(), json!({ "reference": ctx.patient }));
    r.insert("date".into(), json!(date));
    r.insert(
        "author".into(),
        json!(authors
            .iter()
            .map(|a| json!({ "reference": a }))
            .collect::<Vec<_>>()),
    );
    let title = doc
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .or_else(|| {
            type_
                .as_ref()
                .and_then(|t| t["coding"][0]["display"].as_str().map(str::to_owned))
        });
    r.insert(
        "title".into(),
        json!(title.unwrap_or_else(|| {
            ctx.report.info(
                "composition.title.assumed",
                "the document has no title; the title was set to a generic one",
            );
            "Clinical document".to_owned()
        })),
    );
    if let Some(c) = doc.confidentiality.as_deref() {
        if CONFIDENTIALITY.contains(&c) {
            r.insert("confidentiality".into(), json!(c));
        } else {
            ctx.report.info(
                "composition.confidentiality.unknown",
                "confidentialityCode is not a v3 Confidentiality code; it was not mapped",
            );
        }
    }
    if let Some(id) = crate::encounter::find_encounter(doc, ctx, resources) {
        r.insert(
            "encounter".into(),
            json!({ "reference": ctx.reference("Encounter", &id) }),
        );
    }
    if let Some(l) = doc.language.as_deref() {
        r.insert("language".into(), json!(l));
    }
    if let Some(c) = doc
        .custodian
        .as_ref()
        .and_then(|o| register_organization(o, ctx))
    {
        r.insert("custodian".into(), json!({ "reference": c }));
    }

    let sections: Vec<Value> = doc
        .sections
        .iter()
        .filter_map(|s| section(s, &by_key, ctx))
        .collect();
    if !sections.is_empty() {
        r.insert("section".into(), json!(sections));
    }
    Some(Value::Object(r))
}

fn statement_keys(s: &Statement, ctx: &mut Context, out: &mut Vec<String>) {
    // Matching must not add report entries of its own.
    let mut scratch = Report::default();
    for id in &s.ids {
        if let Some(i) = map_identifier(id, ctx.terminology, &mut scratch) {
            out.extend(keys(&json!([i])));
        }
    }
    for rel in &s.related {
        statement_keys(&rel.statement, ctx, out);
    }
}

fn section(
    s: &Section,
    by_key: &BTreeMap<String, Vec<String>>,
    ctx: &mut Context,
) -> Option<Value> {
    let mut found = Vec::new();
    for e in &s.entries {
        statement_keys(&e.statement, ctx, &mut found);
    }
    // Keep first-seen order; the set makes the duplicate check constant time.
    let mut refs: Vec<String> = Vec::new();
    let mut seen: BTreeSet<&String> = BTreeSet::new();
    for k in found {
        for reference in by_key.get(&k).into_iter().flatten() {
            if seen.insert(reference) {
                refs.push(reference.clone());
            }
        }
    }
    let children: Vec<Value> = s
        .sections
        .iter()
        .filter_map(|c| section(c, by_key, ctx))
        .collect();
    let text = s.text.as_deref().map(str::trim).filter(|t| !t.is_empty());

    if text.is_none() && refs.is_empty() && children.is_empty() {
        ctx.report.info(
            "composition.section.empty",
            "a section has no narrative, mapped entries or subsections and was left out of the Composition",
        );
        return None;
    }

    let mut o = Map::new();
    if let Some(t) = s.title.as_deref().filter(|t| !t.trim().is_empty()) {
        o.insert("title".into(), json!(t));
    }
    if let Some(c) = s
        .code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
    {
        o.insert("code".into(), c);
    }
    if let Some(t) = text {
        o.insert(
            "text".into(),
            json!({ "status": "generated", "div": narrative_div(t) }),
        );
    }
    if !refs.is_empty() {
        o.insert(
            "entry".into(),
            json!(refs
                .iter()
                .map(|r| json!({ "reference": r }))
                .collect::<Vec<_>>()),
        );
    } else if s.null_flavor.is_some() && children.is_empty() {
        o.insert(
            "emptyReason".into(),
            json!({ "coding": [{ "system": EMPTY_REASON_SYSTEM, "code": "unavailable" }] }),
        );
    }
    if !children.is_empty() {
        o.insert("section".into(), json!(children));
    }
    Some(Value::Object(o))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccda_terminology::BuiltinTerminology;

    const DOC: &str = r#"<ClinicalDocument xmlns="urn:hl7-org:v3">
  <id root="2.16.840.1.113883.19.5" extension="DOC-1"/>
  <code code="34133-9" codeSystem="2.16.840.1.113883.6.1" displayName="Summarization of episode note"/>
  <title>Continuity of Care</title>
  <effectiveTime value="20250102120000-0500"/>
  <confidentialityCode code="N" codeSystem="2.16.840.1.113883.5.25"/>
  <languageCode code="en-US"/>
  <author><time value="20250102"/><assignedAuthor>
    <id root="2.16.840.1.113883.4.6" extension="1234567893"/>
    <assignedPerson><name><given>Pat</given><family>Doe</family></name></assignedPerson>
  </assignedAuthor></author>
  <custodian><assignedCustodian><representedCustodianOrganization>
    <id root="1.2.3" extension="C-1"/><name>Example Custodian</name>
  </representedCustodianOrganization></assignedCustodian></custodian>
  <component><structuredBody>
    <component><section><code code="48765-2" codeSystem="2.16.840.1.113883.6.1"/><title>Allergies</title>
      <text>Penicillin</text>
      <entry><act classCode="ACT" moodCode="EVN"><id root="1.2.3" extension="A-1"/></act></entry>
    </section></component>
    <component><section><code code="11450-4" codeSystem="2.16.840.1.113883.6.1"/><title>Problems</title></section></component>
  </structuredBody></component>
</ClinicalDocument>"#;

    fn run(xml: &str, resources: &[Value]) -> (Option<Value>, Report) {
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let c = map_composition(&d, &mut ctx, resources);
        (c, report)
    }

    fn allergy() -> Value {
        json!({ "resourceType": "AllergyIntolerance", "id": "allergyintolerance-1",
                "identifier": [{ "system": "urn:oid:1.2.3", "value": "A-1" }] })
    }

    #[test]
    fn header_fields_and_sections() {
        let (c, _) = run(DOC, &[allergy()]);
        let c = c.unwrap();
        assert_eq!(c["status"], "final");
        assert_eq!(c["title"], "Continuity of Care");
        assert_eq!(c["date"], "2025-01-02T12:00:00-05:00");
        assert_eq!(c["confidentiality"], "N");
        assert_eq!(c["language"], "en-US");
        assert_eq!(c["type"]["coding"][0]["code"], "34133-9");
        assert!(c["custodian"]["reference"].is_string());
        assert_eq!(c["section"][0]["title"], "Allergies");
        assert!(c["section"][0]["entry"][0]["reference"]
            .as_str()
            .unwrap()
            .starts_with("urn:uuid:"));
        // The Problems section has nothing to show and is left out.
        assert_eq!(c["section"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn missing_author_means_no_composition() {
        let xml = DOC.replace("<author>", "<x>").replace("</author>", "</x>");
        let (c, report) = run(&xml, &[]);
        assert!(c.is_none());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "composition.author.missing"));
    }

    #[test]
    fn unknown_confidentiality_is_reported() {
        let xml = DOC.replace(
            "code=\"N\" codeSystem=\"2.16.840.1.113883.5.25\"",
            "code=\"Z\"",
        );
        let (c, report) = run(&xml, &[]);
        assert!(c.unwrap().get("confidentiality").is_none());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "composition.confidentiality.unknown"));
    }
}
