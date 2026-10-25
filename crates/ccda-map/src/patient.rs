//! Header `recordTarget/patientRole/patient` to a FHIR `Patient`.

use ccda_model::ClinicalDocument;
use serde_json::{json, Map, Value};

use crate::{hl7_date_to_fhir_date, Report};

/// Map the document's patient to a FHIR `Patient` resource.
///
/// Scope today: name, gender and birth date. Identifiers, address, telecom, race, ethnicity and
/// the rest of USCDI Patient Demographics come with the coverage matrix.
pub fn map_patient(doc: &ClinicalDocument, report: &mut Report) -> Value {
    let patient = &doc.patient;
    let mut resource = Map::new();
    resource.insert("resourceType".into(), json!("Patient"));

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
/// NOTE: verify against the IG's "Administrative Gender to Patient gender" ConceptMap when the
/// coverage matrix is built.
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
    use ccda_model::Patient;

    fn doc_with(patient: Patient) -> ClinicalDocument {
        ClinicalDocument {
            id: None,
            effective_time: None,
            patient,
        }
    }

    #[test]
    fn maps_name_gender_and_birth_date() {
        let doc = doc_with(Patient {
            given: vec!["Alice".into(), "Q".into()],
            family: Some("Example".into()),
            birth_time: Some("19700101".into()),
            gender_code: Some("F".into()),
        });
        let mut report = Report::default();
        let resource = map_patient(&doc, &mut report);
        assert_eq!(resource["resourceType"], "Patient");
        assert_eq!(resource["name"][0]["family"], "Example");
        assert_eq!(resource["name"][0]["given"], json!(["Alice", "Q"]));
        assert_eq!(resource["gender"], "female");
        assert_eq!(resource["birthDate"], "1970-01-01");
        assert!(report.issues.is_empty());
    }

    #[test]
    fn unrecognized_gender_becomes_unknown_with_a_warning() {
        let doc = doc_with(Patient {
            gender_code: Some("X".into()),
            ..Patient::default()
        });
        let mut report = Report::default();
        let resource = map_patient(&doc, &mut report);
        assert_eq!(resource["gender"], "unknown");
        assert!(report.has_warnings());
        assert_eq!(report.issues[0].code, "patient.gender.unrecognized");
        assert!(!report.issues[0].message.contains('X'));
    }

    #[test]
    fn invalid_birth_time_is_omitted_with_a_warning() {
        let doc = doc_with(Patient {
            birth_time: Some("19701301".into()),
            ..Patient::default()
        });
        let mut report = Report::default();
        let resource = map_patient(&doc, &mut report);
        assert!(resource.get("birthDate").is_none());
        assert_eq!(report.issues[0].code, "patient.birthTime.invalid");
    }

    #[test]
    fn empty_patient_yields_a_bare_resource() {
        let mut report = Report::default();
        let resource = map_patient(&doc_with(Patient::default()), &mut report);
        assert_eq!(resource, json!({ "resourceType": "Patient" }));
    }
}
