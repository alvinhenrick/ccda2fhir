//! C-CDA coded values (`CD`/`CE`) to FHIR `CodeableConcept`, following the IG's "CDA Coding to
//! FHIR CodeableConcept" table.

use ccda_model::Code;
use ccda_terminology::{is_oid, Terminology};
use serde_json::{json, Map, Value};

use crate::Report;

/// One `Coding` for a code, or `None` if it has no code value. The second value is the document's
/// own display when it was replaced by the official one.
fn coding(
    code: &Code,
    terminology: &dyn Terminology,
    report: &mut Report,
) -> Option<(Value, Option<String>)> {
    // A code with stray whitespace around it is not a valid code (LOINC rejects '46008-9 ').
    let value = code.code.as_deref()?.trim();
    if value.is_empty() {
        return None;
    }
    let mut coding = Map::new();
    match code.code_system.as_deref() {
        Some(oid) if is_oid(oid) => {
            coding.insert("system".into(), json!(terminology.system_for_oid(oid)));
        }
        Some(_) => report.warn(
            "coding.system.invalid",
            "a codeSystem is not an OID; the coding system was omitted",
        ),
        None => report.warn(
            "coding.system.missing",
            "a code has no codeSystem; the coding system was omitted",
        ),
    }
    coding.insert("code".into(), json!(value));
    let mut replaced = None;
    if let Some(display) = &code.display_name {
        let official = coding
            .get("system")
            .and_then(Value::as_str)
            .and_then(|system| terminology.official_display(system, value));
        match official.as_deref() {
            Some(o) if o != display => {
                report.info(
                    "coding.display.replaced",
                    "a display name differed from the official one; the official display was used and the original kept as text",
                );
                coding.insert("display".into(), json!(o));
                replaced = Some(display.clone());
            }
            _ => {
                coding.insert("display".into(), json!(display));
            }
        }
    }
    Some((Value::Object(coding), replaced))
}

fn is_nucc_grouping(code: &Code, terminology: &dyn Terminology) -> bool {
    let (Some(value), Some(oid)) = (code.code.as_deref(), code.code_system.as_deref()) else {
        return false;
    };
    let v = value.trim().as_bytes();
    v.len() == 10
        && v[..9].iter().all(u8::is_ascii_digit)
        && v[9] == b'X'
        && is_oid(oid)
        && terminology.system_for_oid(oid) == "http://nucc.org/provider-taxonomy"
}

/// Map a coded value (and its translations) to a FHIR `CodeableConcept`.
///
/// Returns `None` if there is no code, no translation with a code, and no text. A coded value
/// with only a `nullFlavor` is the caller's to handle (a data-absent-reason on the element).
pub fn map_codeable_concept(
    code: &Code,
    terminology: &dyn Terminology,
    report: &mut Report,
) -> Option<Value> {
    // A NUCC grouping code (nine digits and an X, like 200000000X) names a whole family of
    // specialties and is not in the code system the validators carry. The words are kept as text.
    let mut grouping_text: Option<String> = None;
    let mapped: Vec<(Value, Option<String>)> = std::iter::once(code)
        .chain(code.translations.iter())
        .filter_map(|c| {
            if is_nucc_grouping(c, terminology) {
                report.info(
                    "coding.nucc-grouping.text-only",
                    "a NUCC grouping code is not a code the validators know; its wording was kept as text and the code left out",
                );
                grouping_text = grouping_text.take().or_else(|| c.display_name.clone());
                return None;
            }
            coding(c, terminology, report)
        })
        .collect();
    // The first replaced display is the wording the document used for this concept.
    let original_display = mapped.iter().find_map(|(_, r)| r.clone());
    let codings: Vec<Value> = mapped.into_iter().map(|(c, _)| c).collect();

    if code.original_text.is_none() && code.original_text_reference.is_some() {
        report.info(
            "coding.original-text.reference-unresolved",
            "an originalText reference was not found in the section narrative; text was omitted",
        );
    }

    if codings.is_empty() && code.original_text.is_none() && grouping_text.is_none() {
        return None;
    }
    let mut concept = Map::new();
    if !codings.is_empty() {
        concept.insert("coding".into(), json!(codings));
    }
    if let Some(text) = code
        .original_text
        .as_ref()
        .or(original_display.as_ref())
        .or(grouping_text.as_ref())
    {
        concept.insert("text".into(), json!(text));
    }
    Some(Value::Object(concept))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccda_terminology::BuiltinTerminology;

    fn code(code: &str, system: &str) -> Code {
        Code {
            code: Some(code.into()),
            code_system: Some(system.into()),
            ..Code::default()
        }
    }

    fn map(c: &Code) -> (Option<Value>, Report) {
        let mut report = Report::default();
        (
            map_codeable_concept(c, &BuiltinTerminology, &mut report),
            report,
        )
    }

    #[test]
    fn nucc_grouping_code_is_kept_as_text_and_codes_are_trimmed() {
        let mut g = code("200000000X", "2.16.840.1.113883.6.101");
        g.display_name = Some("Allopathic & Osteopathic Physicians".into());
        let (v, r) = map(&g);
        assert_eq!(
            v.unwrap(),
            json!({ "text": "Allopathic & Osteopathic Physicians" })
        );
        assert!(r
            .issues
            .iter()
            .any(|i| i.code == "coding.nucc-grouping.text-only"));
        let (v, _) = map(&code("46008-9 ", "2.16.840.1.113883.6.1"));
        assert_eq!(v.unwrap()["coding"][0]["code"], "46008-9");
    }

    #[test]
    fn known_system_uses_the_uri_never_urn_oid() {
        let (v, r) = map(&code("8480-6", "2.16.840.1.113883.6.1"));
        assert_eq!(
            v.unwrap(),
            json!({ "coding": [{ "system": "http://loinc.org", "code": "8480-6" }] })
        );
        assert!(r.issues.is_empty());
    }

    #[test]
    fn unknown_system_falls_back_to_urn_oid() {
        let (v, _) = map(&code("X", "2.16.840.1.113883.4.123456789"));
        assert_eq!(
            v.unwrap()["coding"][0]["system"],
            "urn:oid:2.16.840.1.113883.4.123456789"
        );
    }

    #[test]
    fn display_text_and_translations_are_kept() {
        let mut c = code("8480-6", "2.16.840.1.113883.6.1");
        c.display_name = Some("Systolic blood pressure".into());
        c.original_text = Some("CCD".into());
        c.translations = vec![code("CCD", "1.2.3.4")];
        let (v, _) = map(&c);
        let v = v.unwrap();
        assert_eq!(v["coding"][0]["display"], "Systolic blood pressure");
        assert_eq!(v["coding"][1]["system"], "urn:oid:1.2.3.4");
        assert_eq!(v["text"], "CCD");
    }

    #[test]
    fn a_wrong_display_for_a_known_code_is_replaced_and_kept_as_text() {
        let mut c = code("29762-2", "2.16.840.1.113883.6.1");
        c.display_name = Some("Social history".into());
        let (v, r) = map(&c);
        let v = v.unwrap();
        assert_eq!(v["coding"][0]["display"], "Social history note");
        assert_eq!(v["text"], "Social history");
        assert!(r.issues.iter().any(|i| i.code == "coding.display.replaced"));
        // The report never carries the document's wording.
        assert!(r
            .issues
            .iter()
            .all(|i| !i.message.contains("Social history")));
    }

    #[test]
    fn a_correct_or_unknown_display_is_left_alone() {
        let mut c = code("29762-2", "2.16.840.1.113883.6.1");
        c.display_name = Some("Social history note".into());
        let (v, r) = map(&c);
        assert_eq!(v.unwrap().get("text"), None);
        assert!(r.issues.is_empty());
        let mut c = code("99999-9", "2.16.840.1.113883.6.1");
        c.display_name = Some("Whatever".into());
        let (v, _) = map(&c);
        assert_eq!(v.unwrap()["coding"][0]["display"], "Whatever");
    }

    #[test]
    fn missing_or_invalid_system_warns_and_omits_the_system() {
        let mut c = Code {
            code: Some("A".into()),
            ..Code::default()
        };
        let (v, r) = map(&c);
        assert!(v.unwrap()["coding"][0].get("system").is_none());
        assert_eq!(r.issues[0].code, "coding.system.missing");

        c.code_system = Some("Secret-system".into());
        let (_, r) = map(&c);
        assert_eq!(r.issues[0].code, "coding.system.invalid");
        assert!(!r.issues[0].message.contains("Secret"));
    }

    #[test]
    fn unresolved_original_text_reference_is_noted() {
        let mut c = code("A", "1.2.3");
        c.original_text_reference = Some("#a1".into());
        let (v, r) = map(&c);
        assert!(v.unwrap().get("text").is_none());
        assert_eq!(
            r.issues[0].code,
            "coding.original-text.reference-unresolved"
        );
    }

    #[test]
    fn null_flavor_only_code_maps_to_nothing() {
        let c = Code {
            null_flavor: Some("UNK".into()),
            ..Code::default()
        };
        assert_eq!(map(&c).0, None);
    }

    #[test]
    fn translation_alone_still_produces_a_concept() {
        let c = Code {
            null_flavor: Some("OTH".into()),
            translations: vec![code("A", "2.16.840.1.113883.6.96")],
            ..Code::default()
        };
        let v = map(&c).0.unwrap();
        assert_eq!(v["coding"][0]["system"], "http://snomed.info/sct");
    }
}
