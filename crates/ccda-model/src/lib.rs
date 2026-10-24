//! Typed, version-neutral model of a C-CDA document, plus a safe parser.
//!
//! Lesson 1 scope: parse the document header (document id, effective time,
//! and the patient's name, birth date and gender) with input limits.
//! Later phases add sections, entries, authors, and so on.

use roxmltree::{Document, Node, ParsingOptions};
use thiserror::Error;

/// The HL7 v3 namespace that C-CDA elements live in.
const HL7_NS: &str = "urn:hl7-org:v3";

/// Limits applied before and during parsing. Safe by default.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Maximum input size in bytes.
    pub max_bytes: usize,
    /// Maximum number of XML nodes the parser will create.
    pub max_nodes: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 50 * 1024 * 1024, // 50 MiB
            max_nodes: 5_000_000,
        }
    }
}

/// Everything that can go wrong while parsing. Messages never include
/// document content, so they are safe to log (no PHI).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("input is {size} bytes, which exceeds the limit of {limit} bytes")]
    TooLarge { size: usize, limit: usize },
    #[error("input is not valid UTF-8")]
    NotUtf8,
    #[error("XML could not be parsed or exceeded a safety limit")]
    Xml,
    #[error("root element is not a ClinicalDocument in the HL7 v3 namespace")]
    NotClinicalDocument,
}

/// A coded or plain identifier: `<id root="..." extension="..."/>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier {
    pub root: String,
    pub extension: Option<String>,
}

/// The patient as described in the header's `recordTarget`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Patient {
    pub given: Vec<String>,
    pub family: Option<String>,
    /// Raw HL7 v3 timestamp, e.g. `19700101` (normalization comes later).
    pub birth_time: Option<String>,
    /// Administrative gender code, e.g. `M` or `F`.
    pub gender_code: Option<String>,
}

/// The parts of a C-CDA document we model so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClinicalDocument {
    pub id: Option<Identifier>,
    pub effective_time: Option<String>,
    pub patient: Patient,
}

/// Parse a C-CDA document with the default [`Limits`].
pub fn parse(xml: &[u8]) -> Result<ClinicalDocument, ParseError> {
    parse_with_limits(xml, &Limits::default())
}

/// Parse a C-CDA document with explicit limits.
pub fn parse_with_limits(xml: &[u8], limits: &Limits) -> Result<ClinicalDocument, ParseError> {
    if xml.len() > limits.max_bytes {
        return Err(ParseError::TooLarge {
            size: xml.len(),
            limit: limits.max_bytes,
        });
    }
    let text = std::str::from_utf8(xml).map_err(|_| ParseError::NotUtf8)?;

    // `allow_dtd: false` makes roxmltree reject any DOCTYPE, which closes the
    // door on entity-expansion attacks. `nodes_limit` bounds memory use.
    let options = ParsingOptions {
        allow_dtd: false,
        nodes_limit: limits.max_nodes,
    };
    let doc = Document::parse_with_options(text, options).map_err(|_| ParseError::Xml)?;

    let root = doc.root_element();
    if !is_hl7(&root, "ClinicalDocument") {
        return Err(ParseError::NotClinicalDocument);
    }

    Ok(ClinicalDocument {
        id: child(&root, "id").map(identifier),
        effective_time: child(&root, "effectiveTime")
            .and_then(|n| n.attribute("value"))
            .map(str::to_owned),
        patient: parse_patient(&root),
    })
}

/// True if `node` is an element named `name` in the HL7 v3 namespace.
fn is_hl7(node: &Node, name: &str) -> bool {
    node.is_element()
        && node.tag_name().name() == name
        && node.tag_name().namespace() == Some(HL7_NS)
}

/// First direct child element with the given HL7 name.
fn child<'a, 'input>(node: &Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|c| is_hl7(c, name))
}

/// Follow a path of child element names, e.g. `["recordTarget", "patientRole"]`.
fn path<'a, 'input>(node: &Node<'a, 'input>, names: &[&str]) -> Option<Node<'a, 'input>> {
    names
        .iter()
        .try_fold(*node, |current, name| child(&current, name))
}

fn identifier(node: Node) -> Identifier {
    Identifier {
        root: node.attribute("root").unwrap_or_default().to_owned(),
        extension: node.attribute("extension").map(str::to_owned),
    }
}

/// Text content of an element, trimmed; `None` if empty.
fn text_of(node: &Node) -> Option<String> {
    let text: String = node
        .descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect();
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn parse_patient(root: &Node) -> Patient {
    let Some(patient) = path(root, &["recordTarget", "patientRole", "patient"]) else {
        return Patient::default();
    };

    let name = child(&patient, "name");
    let given = name
        .iter()
        .flat_map(|n| n.children().filter(|c| is_hl7(c, "given")))
        .filter_map(|c| text_of(&c))
        .collect();
    let family = name
        .and_then(|n| child(&n, "family"))
        .and_then(|n| text_of(&n));

    Patient {
        given,
        family,
        birth_time: child(&patient, "birthTime")
            .and_then(|n| n.attribute("value"))
            .map(str::to_owned),
        gender_code: child(&patient, "administrativeGenderCode")
            .and_then(|n| n.attribute("code"))
            .map(str::to_owned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ClinicalDocument xmlns="urn:hl7-org:v3">
  <id root="2.16.840.1.113883.19.5" extension="DOC-1"/>
  <effectiveTime value="20250102120000-0500"/>
  <recordTarget>
    <patientRole>
      <patient>
        <name><given>Alice</given><given>Q</given><family>Example</family></name>
        <administrativeGenderCode code="F" codeSystem="2.16.840.1.113883.5.1"/>
        <birthTime value="19700101"/>
      </patient>
    </patientRole>
  </recordTarget>
</ClinicalDocument>"#;

    #[test]
    fn parses_header_and_patient() {
        let doc = parse(SAMPLE.as_bytes()).expect("sample should parse");
        assert_eq!(
            doc.id,
            Some(Identifier {
                root: "2.16.840.1.113883.19.5".into(),
                extension: Some("DOC-1".into())
            })
        );
        assert_eq!(doc.effective_time.as_deref(), Some("20250102120000-0500"));
        assert_eq!(doc.patient.given, vec!["Alice", "Q"]);
        assert_eq!(doc.patient.family.as_deref(), Some("Example"));
        assert_eq!(doc.patient.birth_time.as_deref(), Some("19700101"));
        assert_eq!(doc.patient.gender_code.as_deref(), Some("F"));
    }

    #[test]
    fn missing_patient_is_not_an_error() {
        let xml =
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><id root="1.2.3"/></ClinicalDocument>"#;
        let doc = parse(xml.as_bytes()).unwrap();
        assert_eq!(doc.patient, Patient::default());
    }

    #[test]
    fn rejects_wrong_root_and_namespace() {
        assert_eq!(
            parse(b"<Other xmlns=\"urn:hl7-org:v3\"/>"),
            Err(ParseError::NotClinicalDocument)
        );
        assert_eq!(
            parse(b"<ClinicalDocument/>"),
            Err(ParseError::NotClinicalDocument)
        );
    }

    #[test]
    fn rejects_dtd_and_entity_bombs() {
        let bomb = r#"<?xml version="1.0"?>
<!DOCTYPE lolz [<!ENTITY a "aaaaaaaaaa"><!ENTITY b "&a;&a;&a;&a;&a;">]>
<ClinicalDocument xmlns="urn:hl7-org:v3">&b;</ClinicalDocument>"#;
        assert_eq!(parse(bomb.as_bytes()), Err(ParseError::Xml));
    }

    #[test]
    fn enforces_size_limit() {
        let limits = Limits {
            max_bytes: 10,
            ..Limits::default()
        };
        assert_eq!(
            parse_with_limits(SAMPLE.as_bytes(), &limits),
            Err(ParseError::TooLarge {
                size: SAMPLE.len(),
                limit: 10
            })
        );
    }

    #[test]
    fn rejects_invalid_utf8() {
        assert_eq!(parse(&[0xff, 0xfe, 0xfd]), Err(ParseError::NotUtf8));
    }

    #[test]
    fn error_messages_contain_no_document_content() {
        let msg = parse(b"<Secret-Patient-Name/>").unwrap_err().to_string();
        assert!(!msg.contains("Secret"));
    }
}
