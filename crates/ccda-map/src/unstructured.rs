//! Unstructured documents (`nonXMLBody`) to a FHIR `DocumentReference`.
//!
//! The embedded file (PDF, text, ...) goes into `content.attachment`. A body that only points at
//! a file outside the document becomes an attachment `url`, and is reported, because the file
//! itself is not part of the input.

use ccda_model::ClinicalDocument;
use serde_json::{json, Map, Value};

use crate::author::register_author_agent;
use crate::note::{base64, is_base64};
use crate::{map_codeable_concept, map_datetime, map_identifier, Context};

const NOTE_CATEGORY_SYSTEM: &str =
    "http://hl7.org/fhir/us/core/CodeSystem/us-core-documentreference-category";

/// Media type from a file name, for a body that only references a file.
fn media_type_of(name: &str) -> Option<&'static str> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "rtf" => "text/rtf",
        "html" | "htm" => "text/html",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "tif" | "tiff" => "image/tiff",
        "doc" => "application/msword",
        _ => return None,
    })
}

/// Map the unstructured body, if the document has one.
pub fn map_unstructured(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let Some(body) = &doc.non_xml_body else {
        return Vec::new();
    };

    let mut attachment = Map::new();
    let data: String = body
        .content
        .as_deref()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if body.representation.as_deref() == Some("B64") {
        if is_base64(&data) {
            attachment.insert("data".into(), json!(data));
        } else if !data.is_empty() {
            ctx.report.warn(
                "unstructured.content.invalid-base64",
                "the unstructured body says it is base64 but it is not; the content was omitted",
            );
        }
    } else if let Some(text) = body.content.as_deref().filter(|t| !t.trim().is_empty()) {
        attachment.insert("data".into(), json!(base64(text.as_bytes())));
    }
    let media = body.media_type.clone().or_else(|| {
        if attachment.contains_key("data") {
            Some("text/plain".to_owned())
        } else {
            body.reference
                .as_deref()
                .and_then(media_type_of)
                .map(str::to_owned)
        }
    });
    if let Some(m) = media {
        attachment.insert("contentType".into(), json!(m));
    }
    if let Some(r) = body.reference.as_deref().filter(|r| !r.trim().is_empty()) {
        attachment.insert("url".into(), json!(r));
        ctx.report.info(
            "unstructured.reference.external",
            "the unstructured body points at a file outside the document; only its name was kept",
        );
    }
    if !attachment.contains_key("data") && !attachment.contains_key("url") {
        ctx.report.warn(
            "unstructured.content.missing",
            "the document has an unstructured body with no content or reference; nothing was mapped",
        );
        return Vec::new();
    }

    let mut r = Map::new();
    r.insert("resourceType".into(), json!("DocumentReference"));
    r.insert("id".into(), json!(ctx.next_id("documentreference")));
    if let Some(id) = doc
        .id
        .as_ref()
        .and_then(|i| map_identifier(i, ctx.terminology, ctx.report))
    {
        r.insert("masterIdentifier".into(), id);
    }
    r.insert("status".into(), json!("current"));
    if let Some(ty) = doc
        .code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
    {
        r.insert("type".into(), ty);
    }
    r.insert(
        "category".into(),
        json!([{ "coding": [{ "system": NOTE_CATEGORY_SYSTEM, "code": "clinical-note", "display": "Clinical Note" }] }]),
    );
    r.insert("subject".into(), json!({ "reference": ctx.patient }));
    if let Some(date) = doc
        .effective_time
        .as_deref()
        .and_then(|t| map_datetime(t, ctx.report))
        .filter(|d| d.contains('T'))
    {
        r.insert("date".into(), json!(date));
    }
    let authors: Vec<Value> = doc
        .authors
        .iter()
        .filter_map(|a| register_author_agent(a, ctx))
        .map(|reference| json!({ "reference": reference }))
        .collect();
    if !authors.is_empty() {
        r.insert("author".into(), json!(authors));
    }
    if let Some(title) = doc.title.as_deref().filter(|t| !t.trim().is_empty()) {
        r.insert("description".into(), json!(title));
    }
    r.insert(
        "content".into(),
        json!([{ "attachment": Value::Object(attachment) }]),
    );
    vec![Value::Object(r)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(body: &str) -> (Vec<Value>, Report) {
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><typeId root="2.16.840.1.113883.1.3"/>
<id root="2.16.840.1.113883.19.5" extension="DOC1"/><title>Consult</title>
<component><nonXMLBody>{body}</nonXMLBody></component></ClinicalDocument>"#
        );
        let doc = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        (map_unstructured(&doc, &mut ctx), report)
    }

    #[test]
    fn embedded_base64_is_kept_with_its_media_type() {
        let (out, _) =
            run(r#"<text mediaType="application/pdf" representation="B64">JVBERi0=</text>"#);
        let att = &out[0]["content"][0]["attachment"];
        assert_eq!(att["contentType"], "application/pdf");
        assert_eq!(att["data"], "JVBERi0=");
        assert_eq!(out[0]["description"], "Consult");
    }

    #[test]
    fn plain_text_is_encoded() {
        let (out, _) = run(r#"<text mediaType="text/plain">hi</text>"#);
        assert_eq!(out[0]["content"][0]["attachment"]["data"], "aGk=");
    }

    #[test]
    fn reference_only_becomes_a_url_and_is_reported() {
        let (out, r) = run(r#"<text><reference value="a.pdf"/></text>"#);
        let att = &out[0]["content"][0]["attachment"];
        assert_eq!(att["url"], "a.pdf");
        assert_eq!(att["contentType"], "application/pdf");
        assert!(r
            .issues
            .iter()
            .any(|i| i.code == "unstructured.reference.external"));
    }

    #[test]
    fn invalid_base64_is_dropped_with_a_warning() {
        let (out, r) = run(r#"<text mediaType="application/pdf" representation="B64">!!</text>"#);
        assert!(out.is_empty());
        assert!(r
            .issues
            .iter()
            .any(|i| i.code == "unstructured.content.invalid-base64"));
    }
}
