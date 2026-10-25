//! Which FHIR profiles the output claims conformance to.
//!
//! The profile set is data, not logic spread over mappers: switching US Core versions, or
//! turning profile claims off, changes this one table.

use serde_json::{json, Value};

/// The set of profiles to declare in `meta.profile`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Profiles {
    /// US Core 6.1.0 (the release that matches USCDI v3).
    #[default]
    UsCore610,
    /// Declare no profiles.
    None,
}

const US_CORE: &str = "http://hl7.org/fhir/us/core/StructureDefinition";

impl Profiles {
    /// The profile URL for a resource, if this set has one.
    #[must_use]
    pub fn profile_for(self, resource: &Value) -> Option<String> {
        if self == Self::None {
            return None;
        }
        let name = match resource["resourceType"].as_str()? {
            "Patient" => "us-core-patient",
            "AllergyIntolerance" => "us-core-allergyintolerance",
            "Immunization" => "us-core-immunization",
            // US Core requires authoredOn and requester; claim it only when both are present.
            "Observation" => observation_profile(resource)?,
            "DiagnosticReport" => {
                let lab = resource["category"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|c| c["coding"].as_array().into_iter().flatten())
                    .any(|c| c["code"] == "LAB");
                let timed = resource["effectiveDateTime"].is_string()
                    || resource["effectivePeriod"].is_object();
                if !(lab && timed && resource["code"]["coding"].is_array()) {
                    return None;
                }
                "us-core-diagnosticreport-lab"
            }
            "MedicationRequest" => {
                if !(resource["authoredOn"].is_string() && resource["requester"].is_object()) {
                    return None;
                }
                "us-core-medicationrequest"
            }
            "Medication" => "us-core-medication",
            // provenance-1: an agent that is a Practitioner needs onBehalfOf. We cannot see the
            // type behind a urn reference, so claim the profile only when every agent has one.
            "Provenance" => {
                let all_on_behalf = resource["agent"].as_array().is_some_and(|a| {
                    !a.is_empty() && a.iter().all(|x| x["onBehalfOf"].is_object())
                });
                if !all_on_behalf {
                    return None;
                }
                "us-core-provenance"
            }
            "Organization" => {
                if !(resource["name"].is_string() && resource["active"].is_boolean()) {
                    return None;
                }
                "us-core-organization"
            }
            // Only claim the profile when its required elements are present: identifier with a
            // system and value, and a name with a family.
            "Practitioner" => {
                let has_identifier = resource["identifier"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|i| i["system"].is_string() && i["value"].is_string());
                let has_family = resource["name"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|n| n["family"].is_string());
                if !(has_identifier && has_family) {
                    return None;
                }
                "us-core-practitioner"
            }
            "Condition" => {
                let encounter_diagnosis = resource["category"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|c| c["coding"].as_array().into_iter().flatten())
                    .any(|c| c["code"] == "encounter-diagnosis");
                if encounter_diagnosis {
                    "us-core-condition-encounter-diagnosis"
                } else {
                    "us-core-condition-problems-health-concerns"
                }
            }
            _ => return None,
        };
        Some(format!("{US_CORE}/{name}"))
    }

    /// Add `meta.profile` to a resource, if the set has a profile for it.
    pub fn apply(self, resource: &mut Value) {
        if let Some(url) = self.profile_for(resource) {
            resource["meta"] = json!({ "profile": [url] });
        }
    }
}

/// The US Core profile of an Observation: vital signs or laboratory result.
fn observation_profile(resource: &Value) -> Option<&'static str> {
    let in_category = |wanted: &str| {
        resource["category"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|c| c["coding"].as_array().into_iter().flatten())
            .any(|c| c["code"] == wanted)
    };
    if in_category("laboratory") {
        let has_value = resource.as_object().is_some_and(|o| {
            o.keys()
                .any(|k| k.starts_with("value") || k == "dataAbsentReason")
        });
        let timed =
            resource["effectiveDateTime"].is_string() || resource["effectivePeriod"].is_object();
        let coded = resource["code"]["coding"].is_array();
        return (has_value && timed && coded).then_some("us-core-observation-lab");
    }
    vital_signs_profile(resource)
}

/// The US Core vital sign profile an Observation meets, or `None`. A specific profile is claimed
/// only when the value uses a unit that profile allows; otherwise the general vital signs profile
/// is claimed when the Observation is complete enough for it.
fn vital_signs_profile(resource: &Value) -> Option<&'static str> {
    let is_vital = resource["category"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|c| c["coding"].as_array().into_iter().flatten())
        .any(|c| c["code"] == "vital-signs");
    if !is_vital {
        return None;
    }
    let has_loinc = |code: &str| {
        resource["code"]["coding"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|c| c["system"] == "http://loinc.org" && c["code"] == code)
    };
    let unit = resource["valueQuantity"]["code"].as_str();
    let ucum = resource["valueQuantity"]["system"] == "http://unitsofmeasure.org";
    let unit_is = |allowed: &[&str]| ucum && unit.is_some_and(|u| allowed.contains(&u));
    let specific = if has_loinc("85354-9") {
        Some("us-core-blood-pressure")
    } else if has_loinc("59408-5") && unit_is(&["%"]) {
        Some("us-core-pulse-oximetry")
    } else if has_loinc("8867-4") && unit_is(&["/min"]) {
        Some("us-core-heart-rate")
    } else if has_loinc("9279-1") && unit_is(&["/min"]) {
        Some("us-core-respiratory-rate")
    } else if has_loinc("8310-5") && unit_is(&["Cel", "[degF]"]) {
        Some("us-core-body-temperature")
    } else if has_loinc("8302-2") && unit_is(&["cm", "[in_i]"]) {
        Some("us-core-body-height")
    } else if has_loinc("29463-7") && unit_is(&["kg", "g", "[lb_av]", "[oz_av]"]) {
        Some("us-core-body-weight")
    } else if has_loinc("39156-5") && unit_is(&["kg/m2"]) {
        Some("us-core-bmi")
    } else if has_loinc("9843-4") && unit_is(&["cm", "[in_i]"]) {
        Some("us-core-head-circumference")
    } else {
        None
    };
    if specific.is_some() {
        return specific;
    }
    let complete = resource["valueQuantity"].is_object()
        || resource["dataAbsentReason"].is_object()
        || resource["component"].is_array()
        || resource["hasMember"].is_array();
    (complete
        && resource["effectiveDateTime"].is_string() | resource["effectivePeriod"].is_object())
    .then_some("us-core-vital-signs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_us_core_profiles() {
        let mut patient = json!({ "resourceType": "Patient" });
        Profiles::UsCore610.apply(&mut patient);
        assert_eq!(
            patient["meta"]["profile"][0],
            "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient"
        );
    }

    #[test]
    fn provenance_is_claimed_only_when_every_agent_acts_on_behalf_of_someone() {
        let with =
            json!({ "resourceType": "Provenance", "agent": [{ "who": {}, "onBehalfOf": {} }] });
        let without = json!({ "resourceType": "Provenance", "agent": [{ "who": {} }] });
        assert!(Profiles::UsCore610.profile_for(&with).is_some());
        assert!(Profiles::UsCore610.profile_for(&without).is_none());
    }

    #[test]
    fn encounter_diagnosis_uses_its_own_profile() {
        let condition = json!({
            "resourceType": "Condition",
            "category": [{ "coding": [{ "code": "encounter-diagnosis" }] }]
        });
        assert!(Profiles::UsCore610
            .profile_for(&condition)
            .unwrap()
            .ends_with("us-core-condition-encounter-diagnosis"));
        let problem = json!({ "resourceType": "Condition" });
        assert!(Profiles::UsCore610
            .profile_for(&problem)
            .unwrap()
            .ends_with("us-core-condition-problems-health-concerns"));
    }

    #[test]
    fn practitioner_profile_is_claimed_only_when_required_elements_exist() {
        let complete = json!({
            "resourceType": "Practitioner",
            "identifier": [{ "system": "http://hl7.org/fhir/sid/us-npi", "value": "1" }],
            "name": [{ "family": "Doe" }]
        });
        assert!(Profiles::UsCore610.profile_for(&complete).is_some());
        let no_identifier =
            json!({ "resourceType": "Practitioner", "name": [{ "family": "Doe" }] });
        assert_eq!(Profiles::UsCore610.profile_for(&no_identifier), None);
        let no_system = json!({
            "resourceType": "Practitioner",
            "identifier": [{ "value": "1" }],
            "name": [{ "family": "Doe" }]
        });
        assert_eq!(Profiles::UsCore610.profile_for(&no_system), None);
    }

    #[test]
    fn organization_profile_needs_name_and_active() {
        let ok = json!({ "resourceType": "Organization", "name": "X", "active": true });
        assert!(Profiles::UsCore610.profile_for(&ok).is_some());
        let no_active = json!({ "resourceType": "Organization", "name": "X" });
        assert_eq!(Profiles::UsCore610.profile_for(&no_active), None);
    }

    #[test]
    fn none_and_unknown_types_declare_nothing() {
        let mut patient = json!({ "resourceType": "Patient" });
        Profiles::None.apply(&mut patient);
        assert!(patient.get("meta").is_none());
        assert_eq!(
            Profiles::UsCore610.profile_for(&json!({ "resourceType": "Basic" })),
            None
        );
    }
}
