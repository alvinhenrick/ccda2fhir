//! Code system helpers.
//!
//! C-CDA identifies code systems by OID; FHIR uses URIs. This crate owns that translation behind a
//! trait so callers can later swap in a richer implementation (for example one backed by a
//! terminology server) without touching the mappers.

/// True if `s` looks like an OID: two or more dot-separated groups of digits.
#[must_use]
pub fn is_oid(s: &str) -> bool {
    s.split('.').count() >= 2
        && s.split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

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

    /// True if the OID names an identifier system such as SSN or NPI (as opposed to a code
    /// system). The IG says the OID itself must never be sent as an identifier value for these.
    fn is_identifier_system(&self, oid: &str) -> bool;
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
    // CDC's own OID for CVX; real documents use both.
    ("2.16.840.1.113883.6.59", "http://hl7.org/fhir/sid/cvx"),
    (
        "2.16.840.1.113883.6.101",
        "http://nucc.org/provider-taxonomy",
    ),
    (
        "2.16.840.1.113883.5.83",
        "http://terminology.hl7.org/CodeSystem/v3-ObservationInterpretation",
    ),
    (
        "2.16.840.1.113883.5.4",
        "http://terminology.hl7.org/CodeSystem/v3-ActCode",
    ),
    (
        "2.16.840.1.113883.6.90",
        "http://hl7.org/fhir/sid/icd-10-cm",
    ),
    ("2.16.840.1.113883.6.69", "http://hl7.org/fhir/sid/ndc"),
    (
        "2.16.840.1.113883.5.8",
        "http://terminology.hl7.org/CodeSystem/v3-ActReason",
    ),
    (
        "2.16.840.1.113883.3.26.1.1",
        "http://ncicb.nci.nih.gov/xml/owl/EVS/Thesaurus.owl",
    ),
    ("2.16.840.1.113883.6.8", "http://unitsofmeasure.org"),
    ("2.16.840.1.113883.6.12", "http://www.ama-assn.org/go/cpt"),
    (
        "2.16.840.1.113883.6.4",
        "http://www.cms.gov/Medicare/Coding/ICD10",
    ),
    (
        "2.16.840.1.113883.6.285",
        "http://www.cms.gov/Medicare/Coding/HCPCSReleaseCodeSets",
    ),
    (
        "2.16.840.1.113883.5.1",
        "http://terminology.hl7.org/CodeSystem/v3-AdministrativeGender",
    ),
    (
        "2.16.840.1.113883.5.1008",
        "http://terminology.hl7.org/CodeSystem/v3-NullFlavor",
    ),
    ("2.16.840.1.113883.4.1", "http://hl7.org/fhir/sid/us-ssn"),
    ("2.16.840.1.113883.4.6", "http://hl7.org/fhir/sid/us-npi"),
];

/// OIDs that identify people or organizations rather than concepts.
const IDENTIFIER_SYSTEMS: &[&str] = &["2.16.840.1.113883.4.1", "2.16.840.1.113883.4.6"];

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

    fn is_identifier_system(&self, oid: &str) -> bool {
        IDENTIFIER_SYSTEMS.contains(&oid)
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
    fn recognizes_oids() {
        assert!(is_oid("2.16.840.1.113883.6.1"));
        assert!(is_oid("1.2"));
        for bad in ["", "1", "1.", ".1", "1..2", "1.a", "urn:oid:1.2", "1.2 "] {
            assert!(!is_oid(bad), "{bad:?}");
        }
    }

    #[test]
    fn ssn_and_npi_are_identifier_systems_with_uris() {
        assert!(BuiltinTerminology.is_identifier_system("2.16.840.1.113883.4.1"));
        assert_eq!(
            BuiltinTerminology.oid_to_uri("2.16.840.1.113883.4.6"),
            Some("http://hl7.org/fhir/sid/us-npi")
        );
        assert!(!BuiltinTerminology.is_identifier_system("2.16.840.1.113883.6.1"));
    }

    #[test]
    fn identifier_systems_all_have_uris() {
        for oid in IDENTIFIER_SYSTEMS {
            assert!(BuiltinTerminology.oid_to_uri(oid).is_some(), "{oid}");
        }
    }

    #[test]
    fn both_cvx_oids_map_to_cvx() {
        for oid in ["2.16.840.1.113883.12.292", "2.16.840.1.113883.6.59"] {
            assert_eq!(
                BuiltinTerminology.oid_to_uri(oid),
                Some("http://hl7.org/fhir/sid/cvx")
            );
        }
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
