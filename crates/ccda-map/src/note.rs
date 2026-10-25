//! Note Activity (`act`, template 4.202) to FHIR `DocumentReference`.
//!
//! Rules come from the IG page "CF-notes". The note body is embedded as base64 in
//! `content.attachment.data`. Narrative the note refers to is flattened to plain text
//! (`text/plain`); the IG also allows XHTML, which needs narrative markup we do not keep.

use ccda_model::{ClinicalDocument, Statement, StatementKind};
use serde_json::{json, Map, Value};

use crate::author::{apply_authors, AuthorFields};
use crate::{map_codeable_concept, map_datetime, map_identifier, Context};

const NOTE_ACTIVITY: &str = "2.16.840.1.113883.10.20.22.4.202";
const LOINC: &str = "2.16.840.1.113883.6.1";
const NOTE_CATEGORY_SYSTEM: &str =
    "http://hl7.org/fhir/us/core/CodeSystem/us-core-documentreference-category";

/// The latest author goes to `author` (wrapped in an array afterwards); the earliest author time
/// is `date` (an instant; kept only if it has a time and offset).
const FIELDS: AuthorFields = AuthorFields {
    recorder: Some("_author"),
    date: "date",
};

/// Map every Note Activity in the document.
pub fn map_notes(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind == StatementKind::Act && s.has_template(NOTE_ACTIVITY) {
                out.push(map_note(s, ctx));
            }
        }
    }
    out
}

/// Standard base64 (RFC 4648) with padding.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(TABLE[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn is_base64(s: &str) -> bool {
    !s.is_empty()
        && s.len().is_multiple_of(4)
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

fn attachment(s: &Statement, ctx: &mut Context) -> Option<Value> {
    let own = s.text_content.as_ref();
    let media = own.and_then(|t| t.media_type.clone());
    let is_b64 = own.and_then(|t| t.representation.as_deref()) == Some("B64");
    if is_b64 {
        // Embedded base64: the media type and data map one to one.
        let data: String = own
            .and_then(|t| t.content.as_deref())
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if !is_base64(&data) {
            ctx.report.warn(
                "note.content.invalid-base64",
                "a note says its content is base64 but it is not; the note body was omitted",
            );
            return None;
        }
        return Some(json!({
            "contentType": media.unwrap_or_else(|| "application/octet-stream".to_owned()),
            "data": data,
        }));
    }
    // Otherwise the body is text, written in place or taken from the section narrative.
    let text = s
        .text
        .as_deref()
        .or_else(|| own.and_then(|t| t.content.as_deref()))
        .filter(|t| !t.trim().is_empty())?;
    Some(json!({
        "contentType": media.unwrap_or_else(|| "text/plain".to_owned()),
        "data": base64(text.as_bytes()),
    }))
}

fn map_note(s: &Statement, ctx: &mut Context) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("DocumentReference"));
    r.insert("id".into(), json!(ctx.next_id("documentreference")));
    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    r.insert("status".into(), json!("current"));

    // The root code is always 34109-9 (Note); a more specific LOINC code is in a translation.
    let specific = s.code.as_ref().and_then(|c| {
        c.translations
            .iter()
            .find(|t| t.code_system.as_deref() == Some(LOINC) && t.code.is_some())
    });
    let ty = specific
        .or(s.code.as_ref())
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report));
    if let Some(ty) = ty {
        r.insert("type".into(), ty);
    } else {
        ctx.report.warn(
            "note.type.missing",
            "a note has no code; type could not be set",
        );
    }
    r.insert(
        "category".into(),
        json!([{ "coding": [{ "system": NOTE_CATEGORY_SYSTEM, "code": "clinical-note", "display": "Clinical Note" }] }]),
    );
    r.insert("subject".into(), json!({ "reference": ctx.patient }));

    let authors: Vec<_> = s.authors.iter().collect();
    apply_authors(&mut r, FIELDS, &authors, ctx);
    if let Some(a) = r.remove("_author") {
        r.insert("author".into(), json!([a]));
    }
    if r.get("date")
        .and_then(Value::as_str)
        .is_some_and(|d| !d.contains('T'))
    {
        r.remove("date");
        ctx.report.info(
            "note.date.imprecise",
            "the note's author time has no time of day and offset, so date (an instant) was omitted",
        );
    }

    match attachment(s, ctx) {
        Some(att) => {
            r.insert("content".into(), json!([{ "attachment": att }]));
        }
        None => ctx.report.warn(
            "note.content.missing",
            "a note has no body text; content could not be set",
        ),
    }

    if let Some(t) = &s.effective_time {
        let mut period = Map::new();
        for (key, raw) in [
            (
                "start",
                t.value
                    .as_deref()
                    .or(t.low.as_ref().and_then(|p| p.value.as_deref())),
            ),
            (
                "end",
                t.value
                    .as_deref()
                    .or(t.high.as_ref().and_then(|p| p.value.as_deref())),
            ),
        ] {
            if let Some(d) = raw.and_then(|v| map_datetime(v, ctx.report)) {
                period.insert(key.into(), json!(d));
            }
        }
        if !period.is_empty() {
            r.insert("context".into(), json!({ "period": period }));
        }
    }
    Value::Object(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(text: &str, extra: &str) -> (Vec<Value>, Report) {
        let xml = format!(
            r##"<ClinicalDocument xmlns="urn:hl7-org:v3">
<component><structuredBody><component><section>
  <code code="34109-9" codeSystem="2.16.840.1.113883.6.1"/>
  <text><paragraph ID="n1">Patient did well.</paragraph></text>
  <entry><act classCode="ACT" moodCode="EVN">
    <templateId root="2.16.840.1.113883.10.20.22.4.202"/>
    <id root="1.2.3" extension="N-1"/>
    <code code="34109-9" codeSystem="2.16.840.1.113883.6.1">
      <translation code="11488-4" codeSystem="2.16.840.1.113883.6.1"/>
    </code>
    {text}
    <statusCode code="completed"/>
    {extra}
  </act></entry>
</section></component></structuredBody></component></ClinicalDocument>"##
        );
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_notes(&d, &mut ctx);
        (out, report)
    }

    #[test]
    fn base64_matches_known_vectors() {
        for (input, want) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
        ] {
            assert_eq!(base64(input.as_bytes()), want);
        }
    }

    #[test]
    fn narrative_reference_becomes_plain_text_content() {
        let (out, _) = run(r##"<text><reference value="#n1"/></text>"##, "");
        let att = &out[0]["content"][0]["attachment"];
        assert_eq!(att["contentType"], "text/plain");
        assert_eq!(att["data"], base64(b"Patient did well."));
        assert_eq!(out[0]["type"]["coding"][0]["code"], "11488-4");
        assert_eq!(out[0]["category"][0]["coding"][0]["code"], "clinical-note");
        assert_eq!(out[0]["status"], "current");
    }

    #[test]
    fn embedded_base64_keeps_its_media_type() {
        let (out, _) = run(
            r##"<text mediaType="application/rtf" representation="B64">e1xydGYxfQ==
<reference value="#n1"/></text>"##,
            "",
        );
        let att = &out[0]["content"][0]["attachment"];
        assert_eq!(att["contentType"], "application/rtf");
        assert_eq!(att["data"], "e1xydGYxfQ==");
    }

    #[test]
    fn invalid_base64_is_dropped_with_a_warning() {
        let (out, report) = run(
            r#"<text mediaType="text/rtf" representation="B64">not base64!</text>"#,
            "",
        );
        assert!(out[0].get("content").is_none());
        let codes: Vec<_> = report.issues.iter().map(|i| i.code).collect();
        assert!(codes.contains(&"note.content.invalid-base64"));
    }

    #[test]
    fn author_and_time_set_author_date_and_period() {
        let extra = r#"<effectiveTime value="20160908"/>
<author><time value="20160908083215-0500"/><assignedAuthor>
  <id root="2.16.840.1.113883.4.6" extension="1234567893"/>
  <assignedPerson><name><family>Specialist</family></name></assignedPerson>
</assignedAuthor></author>"#;
        let (out, _) = run(r##"<text><reference value="#n1"/></text>"##, extra);
        assert_eq!(out[0]["date"], "2016-09-08T08:32:15-05:00");
        assert!(out[0]["author"][0]["reference"].is_string());
        assert_eq!(out[0]["context"]["period"]["start"], "2016-09-08");
    }
}
