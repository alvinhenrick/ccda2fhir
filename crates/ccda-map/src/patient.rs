//! Header `recordTarget/patientRole` to a FHIR `Patient`.

use ccda_model::ClinicalDocument;
use ccda_terminology::Terminology;
use serde_json::{json, Map, Value};

use crate::{hl7_date_to_fhir_date, map_identifier, Report};

/// The `id` of the one Patient a document describes. Other resources refer to it.
pub const PATIENT_ID: &str = "patient-1";

/// Map the document's patient to a FHIR `Patient` resource.
///
/// Scope today: identifiers, name, gender and birth date. Address, telecom, race, ethnicity and
/// the rest of USCDI Patient Demographics come with the coverage matrix.
pub fn map_patient(
    doc: &ClinicalDocument,
    terminology: &dyn Terminology,
    report: &mut Report,
) -> Value {
    let patient = &doc.patient;
    let mut resource = Map::new();
    resource.insert("resourceType".into(), json!("Patient"));
    resource.insert("id".into(), json!(PATIENT_ID));

    let identifiers: Vec<Value> = patient
        .identifiers
        .iter()
        .filter_map(|id| map_identifier(id, terminology, report))
        .collect();
    if !identifiers.is_empty() {
        resource.insert("identifier".into(), json!(identifiers));
    }

    if patient.family.is_some() || !patient.given.is_empty() {
        let mut name = Map::new();
        if let Some(family) = &patient.family {
            name.insert("family".into(), json!(family));
        }
        if !patient.given.is_empty() {
            name.insert("given".into(), json!(patient.given));
        }
        resource.insert("name".into(), json!([name]));
    }

    if let Some(code) = &patient.gender_code {
        resource.insert("gender".into(), json!(map_gender(code, report)));
    }

    if let Some(raw) = &patient.birth_time {
        match hl7_date_to_fhir_date(raw) {
            Some(date) => {
                resource.insert("birthDate".into(), json!(date));
            }
            None => report.warn(
                "patient.birthTime.invalid",
                "birthTime is not a valid HL7 timestamp; birthDate was omitted",
            ),
        }
    }

    Value::Object(resource)
}

/// HL7 v3 AdministrativeGender to FHIR `Patient.gender`.
///
/// M and F are equivalent; UN maps to `other` ("wider"). Source: the IG's ConceptMap
/// `ConceptMap-CF-AdministrativeGender`. Codes outside that map are our own fallback.
fn map_gender(code: &str, report: &mut Report) -> &'static str {
    match code {
        "M" => "male",
        "F" => "female",
        "UN" => "other",
        _ => {
            report.warn(
                "patient.gender.unrecognized",
                "administrativeGenderCode is not M, F or UN; gender was set to unknown",
            );
            "unknown"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccda_model::{Identifier, Patient};
    use ccda_terminology::BuiltinTerminology;

    fn doc_with(patient: Patient) -> ClinicalDocument {
        ClinicalDocument {
            id: None,
            code: None,
            effective_time: None,
            patient,
            sections: Vec::new(),
        }
    }

    fn map(patient: Patient) -> (Value, Report) {
        let mut report = Report::default();
        let resource = map_patient(&doc_with(patient), &BuiltinTerminology, &mut report);
        (resource, report)
    }

    #[test]
    fn maps_identifiers_name_gender_and_birth_date() {
        let (resource, report) = map(Patient {
            given: vec!["Alice".into(), "Q".into()],
            family: Some("Example".into()),
            birth_time: Some("19700101".into()),
            gender_code: Some("F".into()),
            identifiers: vec![Identifier {
                root: Some("2.16.840.1.113883.19.5".into()),
                extension: Some("MRN-1".into()),
                null_flavor: None,
            }],
        });
        assert_eq!(resource["resourceType"], "Patient");
        assert_eq!(
            resource["identifier"][0],
            json!({ "system": "urn:oid:2.16.840.1.113883.19.5", "value": "MRN-1" })
        );
        assert_eq!(resource["name"][0]["family"], "Example");
        assert_eq!(resource["name"][0]["given"], json!(["Alice", "Q"]));
        assert_eq!(resource["gender"], "female");
        assert_eq!(resource["birthDate"], "1970-01-01");
        assert!(report.issues.is_empty());
    }

    #[test]
    fn gender_follows_the_ig_concept_map() {
        for (code, expected) in [("M", "male"), ("F", "female"), ("UN", "other")] {
            let (resource, report) = map(Patient {
                gender_code: Some(code.into()),
                ..Patient::default()
            });
            assert_eq!(resource["gender"], expected);
            assert!(report.issues.is_empty());
        }
    }

    #[test]
    fn unrecognized_gender_becomes_unknown_with_a_warning() {
        let (resource, report) = map(Patient {
            gender_code: Some("X".into()),
            ..Patient::default()
        });
        assert_eq!(resource["gender"], "unknown");
        assert!(report.has_warnings());
        assert_eq!(report.issues[0].code, "patient.gender.unrecognized");
        assert!(!report.issues[0].message.contains('X'));
    }

    #[test]
    fn invalid_birth_time_is_omitted_with_a_warning() {
        let (resource, report) = map(Patient {
            birth_time: Some("19701301".into()),
            ..Patient::default()
        });
        assert!(resource.get("birthDate").is_none());
        assert_eq!(report.issues[0].code, "patient.birthTime.invalid");
    }

    #[test]
    fn unusable_identifiers_are_dropped_with_a_warning() {
        let (resource, report) = map(Patient {
            identifiers: vec![Identifier::default()],
            ..Patient::default()
        });
        assert!(resource.get("identifier").is_none());
        assert_eq!(report.issues[0].code, "identifier.empty");
    }

    #[test]
    fn empty_patient_yields_a_bare_resource() {
        let (resource, report) = map(Patient::default());
        assert_eq!(
            resource,
            json!({ "resourceType": "Patient", "id": PATIENT_ID })
        );
        assert!(report.issues.is_empty());
    }
}
