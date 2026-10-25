//! Typed, version-neutral model of a C-CDA document, plus a safe parser.
//!
//! The parser enforces input limits, refuses DTDs, and produces plain Rust data: header,
//! sections, and a generic tree of clinical statements. It does not interpret clinical meaning;
//! that is the mappers' job.

mod body;
mod header;
mod types;
mod xml;

use roxmltree::{Document, ParsingOptions};
use thiserror::Error;

pub use body::{Entry, Link, Related, Section, Statement, StatementKind};
pub use header::Patient;
pub use types::{Code, Identifier, Quantity, Scalar, TemplateId, Time, TimePoint, Value};

use types::{identifier, parse_code, Narrative};
use xml::{child, is_hl7};

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
    #[error("sections or clinical statements are nested deeper than the supported limit")]
    TooDeep,
}

/// A parsed C-CDA document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClinicalDocument {
    pub id: Option<Identifier>,
    /// The document type code (for example LOINC 34133-9 for a CCD).
    pub code: Option<Code>,
    pub effective_time: Option<String>,
    pub patient: Patient,
    /// Top-level sections of the structured body, in document order.
    pub sections: Vec<Section>,
}

impl ClinicalDocument {
    /// Every section, including nested subsections, in document order (parents first).
    #[must_use]
    pub fn all_sections(&self) -> Vec<&Section> {
        fn walk<'a>(sections: &'a [Section], out: &mut Vec<&'a Section>) {
            for section in sections {
                out.push(section);
                walk(&section.sections, out);
            }
        }
        let mut out = Vec::new();
        walk(&self.sections, &mut out);
        out
    }
}

/// Parse a C-CDA document with the default [`Limits`].
///
/// # Errors
///
/// See [`ParseError`].
pub fn parse(xml: &[u8]) -> Result<ClinicalDocument, ParseError> {
    parse_with_limits(xml, &Limits::default())
}

/// Parse a C-CDA document with explicit limits.
///
/// # Errors
///
/// See [`ParseError`].
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

    let header_narrative = Narrative::new();
    Ok(ClinicalDocument {
        id: child(&root, "id").map(|n| identifier(&n)),
        code: child(&root, "code").map(|n| parse_code(&n, &header_narrative)),
        effective_time: child(&root, "effectiveTime").and_then(|n| xml::attr(&n, "value")),
        patient: header::parse_patient(&root),
        sections: body::parse_body(&root)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ClinicalDocument xmlns="urn:hl7-org:v3">
  <id root="2.16.840.1.113883.19.5" extension="DOC-1"/>
  <code code="34133-9" codeSystem="2.16.840.1.113883.6.1" displayName="Summarization of episode note">
    <originalText>Continuity of Care Document</originalText>
    <translation code="CCD" codeSystem="1.2.3.4" displayName="CCD"/>
  </code>
  <effectiveTime value="20250102120000-0500"/>
  <recordTarget>
    <patientRole>
      <id root="2.16.840.1.113883.19.5" extension="MRN-1"/>
      <id nullFlavor="NI"/>
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
                root: Some("2.16.840.1.113883.19.5".into()),
                extension: Some("DOC-1".into()),
                null_flavor: None,
            })
        );
        assert_eq!(doc.effective_time.as_deref(), Some("20250102120000-0500"));
        assert_eq!(doc.patient.given, vec!["Alice", "Q"]);
        assert_eq!(doc.patient.family.as_deref(), Some("Example"));
        assert_eq!(doc.patient.birth_time.as_deref(), Some("19700101"));
        assert_eq!(doc.patient.gender_code.as_deref(), Some("F"));
    }

    #[test]
    fn parses_document_code_with_translation_and_original_text() {
        let doc = parse(SAMPLE.as_bytes()).unwrap();
        let code = doc.code.expect("document code");
        assert_eq!(code.code.as_deref(), Some("34133-9"));
        assert_eq!(code.code_system.as_deref(), Some("2.16.840.1.113883.6.1"));
        assert_eq!(
            code.original_text.as_deref(),
            Some("Continuity of Care Document")
        );
        assert_eq!(code.translations.len(), 1);
        assert_eq!(code.translations[0].code.as_deref(), Some("CCD"));
    }

    #[test]
    fn parses_patient_identifiers_including_null_flavor() {
        let doc = parse(SAMPLE.as_bytes()).unwrap();
        let ids = doc.patient.identifiers;
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].extension.as_deref(), Some("MRN-1"));
        assert_eq!(ids[1].root, None);
        assert_eq!(ids[1].null_flavor.as_deref(), Some("NI"));
    }

    #[test]
    fn original_text_reference_is_kept_separately() {
        let xml = r##"<ClinicalDocument xmlns="urn:hl7-org:v3">
  <code code="1" codeSystem="2.3"><originalText><reference value="#a1"/></originalText></code>
</ClinicalDocument>"##;
        let code = parse(xml.as_bytes()).unwrap().code.unwrap();
        assert_eq!(code.original_text, None);
        assert_eq!(code.original_text_reference.as_deref(), Some("#a1"));
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

    // Synthetic problem list modelled on the IG's example (no real patient data).
    const PROBLEMS: &str = r##"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <component><structuredBody>
    <component><section>
      <templateId root="2.16.840.1.113883.10.20.22.2.5.1" extension="2015-08-01"/>
      <code code="11450-4" codeSystem="2.16.840.1.113883.6.1"/>
      <title>Problems</title>
      <text><table><tbody><tr>
        <td ID="prob1">Essential
           hypertension</td><td ID="stat1">Active</td>
      </tr></tbody></table></text>
      <entry typeCode="DRIV">
        <act classCode="ACT" moodCode="EVN">
          <templateId root="2.16.840.1.113883.10.20.22.4.3" extension="2015-08-01"/>
          <id root="1.2.3" extension="C1"/>
          <code code="CONC" codeSystem="2.16.840.1.113883.5.6"/>
          <statusCode code="active"/>
          <effectiveTime><low value="20120806"/><high nullFlavor="UNK"/></effectiveTime>
          <entryRelationship typeCode="SUBJ">
            <observation classCode="OBS" moodCode="EVN" negationInd="false">
              <templateId root="2.16.840.1.113883.10.20.22.4.4" extension="2015-08-01"/>
              <code code="55607006" codeSystem="2.16.840.1.113883.6.96"/>
              <text><reference value="#prob1"/></text>
              <value xsi:type="CD" code="59621000" codeSystem="2.16.840.1.113883.6.96">
                <originalText><reference value="#prob1"/></originalText>
              </value>
              <entryRelationship typeCode="REFR">
                <observation classCode="OBS" moodCode="EVN">
                  <templateId root="2.16.840.1.113883.10.20.22.4.6"/>
                  <code code="33999-4" codeSystem="2.16.840.1.113883.6.1"/>
                  <value xsi:type="CD" code="55561003" codeSystem="2.16.840.1.113883.6.96"/>
                </observation>
              </entryRelationship>
            </observation>
          </entryRelationship>
        </act>
      </entry>
      <component><section><title>Sub</title></section></component>
    </section></component>
  </structuredBody></component>
</ClinicalDocument>"##;

    #[test]
    fn parses_sections_entries_and_nested_statements() {
        let doc = parse(PROBLEMS.as_bytes()).unwrap();
        assert_eq!(doc.sections.len(), 1);
        let section = &doc.sections[0];
        assert!(section.has_template("2.16.840.1.113883.10.20.22.2.5.1"));
        assert_eq!(section.title.as_deref(), Some("Problems"));
        assert_eq!(
            section.code.as_ref().unwrap().code.as_deref(),
            Some("11450-4")
        );
        assert_eq!(section.sections.len(), 1);

        let concern = &section.entries[0].statement;
        assert_eq!(section.entries[0].type_code.as_deref(), Some("DRIV"));
        assert_eq!(concern.kind, StatementKind::Act);
        assert!(concern.has_template("2.16.840.1.113883.10.20.22.4.3"));
        assert_eq!(concern.status_code.as_deref(), Some("active"));
        let time = concern.effective_time.as_ref().unwrap();
        assert_eq!(
            time.low.as_ref().unwrap().value.as_deref(),
            Some("20120806")
        );
        assert_eq!(
            time.high.as_ref().unwrap().null_flavor.as_deref(),
            Some("UNK")
        );

        let observation = concern.related_by("SUBJ").next().unwrap();
        assert!(observation.has_template("2.16.840.1.113883.10.20.22.4.4"));
        assert!(!observation.negation_ind);
        assert_eq!(observation.related_by("REFR").count(), 1);
        assert_eq!(observation.related_by("SUBJ").count(), 0);
    }

    #[test]
    fn resolves_narrative_references_and_collapses_whitespace() {
        let doc = parse(PROBLEMS.as_bytes()).unwrap();
        let observation = doc.sections[0].entries[0]
            .statement
            .related_by("SUBJ")
            .next()
            .unwrap();
        assert_eq!(observation.text.as_deref(), Some("Essential hypertension"));
        let Value::Coded(code) = &observation.values[0] else {
            panic!("expected a coded value");
        };
        assert_eq!(code.code.as_deref(), Some("59621000"));
        assert_eq!(
            code.original_text.as_deref(),
            Some("Essential hypertension")
        );
        assert_eq!(code.original_text_reference.as_deref(), Some("#prob1"));
    }

    #[test]
    fn unresolvable_reference_leaves_text_empty_but_keeps_the_reference() {
        let xml = PROBLEMS.replace("#prob1", "#missing");
        let doc = parse(xml.as_bytes()).unwrap();
        let observation = doc.sections[0].entries[0]
            .statement
            .related_by("SUBJ")
            .next()
            .unwrap();
        assert_eq!(observation.text, None);
        let Value::Coded(code) = &observation.values[0] else {
            panic!("expected a coded value");
        };
        assert_eq!(code.original_text, None);
        assert_eq!(code.original_text_reference.as_deref(), Some("#missing"));
    }

    #[test]
    fn values_are_typed_by_xsi_type() {
        let xml = r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <component><structuredBody><component><section>
    <entry><observation classCode="OBS" moodCode="EVN">
      <value xsi:type="PQ" value="5.4" unit="mg/dL"/>
      <value xsi:type="INT" value="3"/>
      <value xsi:type="ST">free text</value>
      <value xsi:type="BL" value="true"/>
      <value xsi:type="IVL_PQ"/>
      <value/>
    </observation></entry>
  </section></component></structuredBody></component>
</ClinicalDocument>"#;
        let doc = parse(xml.as_bytes()).unwrap();
        let values = &doc.sections[0].entries[0].statement.values;
        assert_eq!(
            values[0],
            Value::Quantity(Quantity {
                value: Some("5.4".into()),
                unit: Some("mg/dL".into()),
                null_flavor: None
            })
        );
        assert!(matches!(&values[1], Value::Integer(s) if s.value.as_deref() == Some("3")));
        assert!(matches!(&values[2], Value::Text(s) if s.value.as_deref() == Some("free text")));
        assert!(matches!(&values[3], Value::Boolean(s) if s.value.as_deref() == Some("true")));
        assert_eq!(
            values[4],
            Value::Other {
                xsi_type: Some("IVL_PQ".into())
            }
        );
        assert_eq!(values[5], Value::Other { xsi_type: None });
    }

    #[test]
    fn negation_and_organizer_components() {
        let xml = r#"<ClinicalDocument xmlns="urn:hl7-org:v3">
  <component><structuredBody><component><section>
    <entry><organizer classCode="BATTERY" moodCode="EVN">
      <component><observation classCode="OBS" moodCode="EVN" negationInd="true"/></component>
      <component><observation classCode="OBS" moodCode="EVN"/></component>
    </organizer></entry>
  </section></component></structuredBody></component>
</ClinicalDocument>"#;
        let doc = parse(xml.as_bytes()).unwrap();
        let organizer = &doc.sections[0].entries[0].statement;
        assert_eq!(organizer.kind, StatementKind::Organizer);
        assert_eq!(organizer.related.len(), 2);
        assert_eq!(organizer.related[0].link, Link::Component);
        assert!(organizer.related[0].statement.negation_ind);
        assert!(!organizer.related[1].statement.negation_ind);
    }

    #[test]
    fn section_with_null_flavor_has_no_entries() {
        let xml = r#"<ClinicalDocument xmlns="urn:hl7-org:v3">
  <component><structuredBody><component>
    <section nullFlavor="NI"><title>Allergies</title></section>
  </component></structuredBody></component>
</ClinicalDocument>"#;
        let doc = parse(xml.as_bytes()).unwrap();
        assert_eq!(doc.sections[0].null_flavor.as_deref(), Some("NI"));
        assert!(doc.sections[0].entries.is_empty());
    }

    #[test]
    fn rejects_runaway_nesting() {
        let depth = body::MAX_DEPTH + 3;
        let open = r#"<entryRelationship typeCode="COMP"><act classCode="ACT" moodCode="EVN">"#
            .repeat(depth);
        let close = "</act></entryRelationship>".repeat(depth);
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><component><structuredBody><component><section>
<entry><act classCode="ACT" moodCode="EVN">{open}{close}</act></entry>
</section></component></structuredBody></component></ClinicalDocument>"#
        );
        assert_eq!(parse(xml.as_bytes()), Err(ParseError::TooDeep));
    }

    #[test]
    fn all_sections_includes_subsections_in_order() {
        let doc = parse(PROBLEMS.as_bytes()).unwrap();
        let titles: Vec<_> = doc
            .all_sections()
            .iter()
            .map(|s| s.title.clone().unwrap())
            .collect();
        assert_eq!(titles, ["Problems", "Sub"]);
    }

    #[test]
    fn parsing_is_deterministic() {
        assert_eq!(
            parse(PROBLEMS.as_bytes()).unwrap(),
            parse(PROBLEMS.as_bytes()).unwrap()
        );
    }
}
