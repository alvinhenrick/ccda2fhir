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
