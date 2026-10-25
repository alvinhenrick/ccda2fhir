//! Comment Activity (`act`, template 4.64) to `note` on the resource of the statement it
//! comments on.
//!
//! The comment is matched to its parent by the parent's ids, the same way the Composition finds
//! the resources of an entry. A comment whose parent did not become a resource that can carry
//! notes is reported and not attached.

use std::collections::BTreeMap;

use ccda_model::{ClinicalDocument, Statement};
use serde_json::{json, Value};

use crate::health_concern::keys;
use crate::{map_datetime, map_identifier, Context, Report};

const COMMENT_ACTIVITY: &str = "2.16.840.1.113883.10.20.22.4.64";

/// Resource types that have a `note` element.
const NOTE_TYPES: &[&str] = &[
    "AllergyIntolerance",
    "CarePlan",
    "Condition",
    "DeviceUseStatement",
    "FamilyMemberHistory",
    "Goal",
    "Immunization",
    "MedicationRequest",
    "MedicationStatement",
    "Observation",
    "Procedure",
    "ServiceRequest",
];

fn annotation(comment: &Statement, ctx: &mut Context) -> Option<Value> {
    let text = comment
        .text
        .as_deref()
        .or_else(|| {
            comment
                .text_content
                .as_ref()
                .and_then(|t| t.content.as_deref())
        })
        .map(str::trim)
        .filter(|t| !t.is_empty())?;
    let mut a = json!({ "text": text });
    if let Some(author) = comment.authors.first() {
        let name = author
            .given
            .iter()
            .map(String::as_str)
            .chain(author.family.as_deref())
            .collect::<Vec<_>>()
            .join(" ");
        if !name.is_empty() {
            a["authorString"] = json!(name);
        }
        if let Some(t) = author
            .time
            .as_deref()
            .and_then(|t| map_datetime(t, ctx.report))
        {
            a["time"] = json!(t);
        }
    }
    Some(a)
}

fn visit(
    s: &Statement,
    index: &BTreeMap<String, usize>,
    resources: &mut [Value],
    ctx: &mut Context,
) {
    let comments: Vec<&Statement> = s
        .related
        .iter()
        .map(|r| &r.statement)
        .filter(|c| c.has_template(COMMENT_ACTIVITY))
        .collect();
    if !comments.is_empty() {
        let mut scratch = Report::default();
        let target = s
            .ids
            .iter()
            .filter_map(|id| map_identifier(id, ctx.terminology, &mut scratch))
            .flat_map(|i| keys(&json!([i])))
            .find_map(|k| index.get(&k).copied());
        for c in comments {
            let Some(a) = annotation(c, ctx) else {
                ctx.report.info(
                    "comment.empty",
                    "a comment activity had no text and was skipped",
                );
                continue;
            };
            match target {
                Some(i) => {
                    let notes = &mut resources[i]["note"];
                    if let Some(arr) = notes.as_array_mut() {
                        arr.push(a);
                    } else {
                        *notes = json!([a]);
                    }
                }
                None => ctx.report.info(
                    "comment.unattached",
                    "a comment activity's parent did not become a resource that can hold notes; the comment was not kept",
                ),
            }
        }
    }
    for r in &s.related {
        visit(&r.statement, index, resources, ctx);
    }
}

/// Attach every Comment Activity to the resource of its parent statement.
pub fn attach_comments(doc: &ClinicalDocument, ctx: &mut Context, resources: &mut [Value]) {
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for (i, r) in resources.iter().enumerate() {
        if r["resourceType"]
            .as_str()
            .is_some_and(|t| NOTE_TYPES.contains(&t))
        {
            for k in keys(&r["identifier"]) {
                index.entry(k).or_insert(i);
            }
        }
    }
    for section in doc.all_sections() {
        for entry in &section.entries {
            visit(&entry.statement, &index, resources, ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccda_terminology::BuiltinTerminology;

    fn run(parent_id: &str) -> (Vec<Value>, Report) {
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><component><structuredBody><component><section><entry>
<act classCode="ACT" moodCode="EVN"><id root="2.16.840.1.113883.19.5" extension="{parent_id}"/>
<entryRelationship typeCode="SUBJ"><act classCode="ACT" moodCode="EVN">
<templateId root="{COMMENT_ACTIVITY}"/><code code="48767-8" codeSystem="2.16.840.1.113883.6.1"/>
<text>Patient declined</text>
<author><time value="20240102"/><assignedAuthor><id root="1.2.3" extension="a"/><assignedPerson><name><given>Pat</given><family>Doe</family></name></assignedPerson></assignedAuthor></author>
</act></entryRelationship></act></entry></section></component></structuredBody></component></ClinicalDocument>"#
        );
        let doc = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let mut resources = vec![json!({
            "resourceType": "Condition",
            "identifier": [{ "system": "urn:oid:2.16.840.1.113883.19.5", "value": "P1" }],
        })];
        attach_comments(&doc, &mut ctx, &mut resources);
        (resources, report)
    }

    #[test]
    fn comment_becomes_a_note_on_the_parent() {
        let (r, _) = run("P1");
        assert_eq!(r[0]["note"][0]["text"], "Patient declined");
        assert_eq!(r[0]["note"][0]["authorString"], "Pat Doe");
        assert_eq!(r[0]["note"][0]["time"], "2024-01-02");
    }

    #[test]
    fn comment_without_a_parent_resource_is_reported() {
        let (r, rep) = run("OTHER");
        assert!(r[0].get("note").is_none());
        assert!(rep.issues.iter().any(|i| i.code == "comment.unattached"));
    }
}
