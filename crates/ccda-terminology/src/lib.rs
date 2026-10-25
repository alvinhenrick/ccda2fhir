//! Code system helpers.
//!
//! C-CDA identifies code systems by OID; FHIR uses URIs. This crate owns that translation behind a
//! trait so callers can later swap in a richer implementation (for example one backed by a
//! terminology server) without touching the mappers.

/// Translates between C-CDA code system OIDs and FHIR system URIs.
pub trait Terminology {
    /// The FHIR system URI for a code system OID, if known.
    fn oid_to_uri(&self, oid: &str) -> Option<&str>;

    /// Like [`Terminology::oid_to_uri`], but falls back to the `urn:oid:` form that FHIR allows
    /// for code systems without a canonical URI.
    fn system_for_oid(&self, oid: &str) -> String {
        match self.oid_to_uri(oid) {
            Some(uri) => uri.to_owned(),
            None => format!("urn:oid:{oid}"),
        }
    }
}

/// OID to URI pairs bundled with the library.
///
/// NOTE: every row must be checked against the HL7 OID registry and the FHIR terminology pages
/// before a release. Tracked in the coverage matrix work.
const BUILTIN: &[(&str, &str)] = &[
    ("2.16.840.1.113883.6.96", "http://snomed.info/sct"),
    ("2.16.840.1.113883.6.1", "http://loinc.org"),
    (
        "2.16.840.1.113883.6.88",
        "http://www.nlm.nih.gov/research/umls/rxnorm",
    ),
    ("2.16.840.1.113883.12.292", "http://hl7.org/fhir/sid/cvx"),
    (
        "2.16.840.1.113883.6.90",
        "http://hl7.org/fhir/sid/icd-10-cm",
    ),
    ("2.16.840.1.113883.6.69", "http://hl7.org/fhir/sid/ndc"),
    ("2.16.840.1.113883.6.8", "http://unitsofmeasure.org"),
    ("2.16.840.1.113883.6.12", "http://www.ama-assn.org/go/cpt"),
    (
        "2.16.840.1.113883.5.1",
        "http://terminology.hl7.org/CodeSystem/v3-AdministrativeGender",
    ),
    (
        "2.16.840.1.113883.5.1008",
        "http://terminology.hl7.org/CodeSystem/v3-NullFlavor",
    ),
];

/// The default, offline [`Terminology`] implementation.
#[derive(Debug, Default, Clone, Copy)]
pub struct BuiltinTerminology;

impl Terminology for BuiltinTerminology {
    fn oid_to_uri(&self, oid: &str) -> Option<&str> {
        BUILTIN
            .iter()
            .find(|(known, _)| *known == oid)
            .map(|(_, uri)| *uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_oid_maps_to_uri() {
        assert_eq!(
            BuiltinTerminology.oid_to_uri("2.16.840.1.113883.6.96"),
            Some("http://snomed.info/sct")
        );
    }

    #[test]
    fn unknown_oid_falls_back_to_urn_oid() {
        assert_eq!(BuiltinTerminology.oid_to_uri("1.2.3"), None);
        assert_eq!(BuiltinTerminology.system_for_oid("1.2.3"), "urn:oid:1.2.3");
    }

    #[test]
    fn builtin_oids_are_unique() {
        for (i, (a, _)) in BUILTIN.iter().enumerate() {
            assert!(
                BUILTIN.iter().skip(i + 1).all(|(b, _)| a != b),
                "duplicate OID {a}"
            );
        }
    }
}
