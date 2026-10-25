//! Convert C-CDA documents to FHIR R4 bundles.
//!
//! ```
//! let xml = br#"<ClinicalDocument xmlns="urn:hl7-org:v3"/>"#;
//! let conversion = ccda2fhir::convert(xml, &ccda2fhir::Options::default()).unwrap();
//! assert_eq!(conversion.bundle.to_json()["resourceType"], "Bundle");
//! ```
//!
//! The library is pure: bytes in, a [`Conversion`] out. It performs no I/O and keeps no state.

use ccda_map::{map_patient, Report};
use ccda_model::Limits;
use ccda_terminology::BuiltinTerminology;
use fhir_out::Bundle;
use thiserror::Error;

/// Settings for a conversion.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Input safety limits applied while parsing.
    pub limits: Limits,
}

/// The result of a successful conversion.
#[derive(Debug, Clone)]
pub struct Conversion {
    /// The FHIR bundle.
    pub bundle: Bundle,
    /// Everything lossy or unexpected that happened on the way.
    pub report: Report,
}

/// Failures that stop a conversion. Data problems are reported in [`Report`] instead.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConvertError {
    /// The input could not be parsed as a C-CDA document.
    #[error("could not parse the C-CDA document: {0}")]
    Parse(#[from] ccda_model::ParseError),
}

/// Convert a C-CDA document to a FHIR R4 bundle.
///
/// # Errors
///
/// Returns [`ConvertError::Parse`] if the input is too large, not UTF-8, not well-formed XML, or
/// not a C-CDA `ClinicalDocument`.
pub fn convert(xml: &[u8], options: &Options) -> Result<Conversion, ConvertError> {
    let document = ccda_model::parse_with_limits(xml, &options.limits)?;

    let mut report = Report::default();
    let mut bundle = Bundle::new();
    bundle.push(map_patient(&document, &BuiltinTerminology, &mut report));

    Ok(Conversion { bundle, report })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SAMPLE: &str = r#"<ClinicalDocument xmlns="urn:hl7-org:v3">
  <recordTarget><patientRole>
    <id root="2.16.840.1.113883.4.1" extension="000-00-0000"/>
    <patient>
    <name><given>Alice</given><family>Example</family></name>
    <administrativeGenderCode code="F"/>
    <birthTime value="19700101"/>
  </patient></patientRole></recordTarget>
</ClinicalDocument>"#;

    #[test]
    fn converts_header_patient_to_a_bundle() {
        let conversion = convert(SAMPLE.as_bytes(), &Options::default()).unwrap();
        let json = conversion.bundle.to_json();
        assert_eq!(json["entry"][0]["resource"]["resourceType"], "Patient");
        assert_eq!(json["entry"][0]["resource"]["birthDate"], "1970-01-01");
        assert_eq!(
            json["entry"][0]["resource"]["identifier"][0]["system"],
            "http://hl7.org/fhir/sid/us-ssn"
        );
        assert!(conversion.report.issues.is_empty());
    }

    #[test]
    fn parse_failures_are_errors_without_document_content() {
        let err = convert(b"<Secret-Name/>", &Options::default()).unwrap_err();
        assert!(matches!(err, ConvertError::Parse(_)));
        assert!(!err.to_string().contains("Secret"));
    }

    #[test]
    fn output_is_deterministic() {
        let a = convert(SAMPLE.as_bytes(), &Options::default()).unwrap();
        let b = convert(SAMPLE.as_bytes(), &Options::default()).unwrap();
        assert_eq!(a.bundle.to_json(), b.bundle.to_json());
        assert_eq!(a.bundle.to_json()["type"], json!("collection"));
    }
}
