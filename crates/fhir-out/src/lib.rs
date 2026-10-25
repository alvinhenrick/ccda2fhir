//! FHIR R4 output building blocks.
//!
//! Resources are plain JSON values for now. That keeps the early phases fast and lets us emit
//! profile-specific elements and extensions freely. Typed resources can replace this later without
//! touching parsing or mapping, because FHIR knowledge stays inside this crate.

use serde_json::{json, Value};

use uuid::Uuid;

fn namespace(seed: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("urn:ccda2fhir:{seed}").as_bytes(),
    )
}

/// The `urn:uuid:` url of a resource, derived from a namespace seed, its type and its id.
/// [`Bundle::url_for`] gives the same value for a bundle created with the same seed.
#[must_use]
pub fn urn_for(seed: &str, resource_type: &str, id: &str) -> String {
    let name = format!("{resource_type}/{id}");
    format!(
        "urn:uuid:{}",
        Uuid::new_v5(&namespace(seed), name.as_bytes())
    )
}

/// A FHIR R4 Bundle of type `collection`.
///
/// Every entry gets a `fullUrl` of the form `urn:uuid:<v5 uuid>`, derived from a namespace seed
/// (the converter uses the source document's id) plus the resource's type and id. The same input
/// always gives the same urls, and two different documents never share them, so bundles from many
/// documents can be loaded into one server. References between resources should use
/// [`Bundle::url_for`], which is what makes them resolve inside the bundle.
#[derive(Debug, Clone, PartialEq)]
pub struct Bundle {
    seed: String,
    entries: Vec<(String, Value)>,
}

impl Default for Bundle {
    fn default() -> Self {
        Self::with_namespace_seed("")
    }
}

impl Bundle {
    /// An empty bundle with the default (empty) namespace seed.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty bundle whose urls are derived from `seed` (for example the document id).
    #[must_use]
    pub fn with_namespace_seed(seed: &str) -> Self {
        Self {
            seed: seed.to_owned(),
            entries: Vec::new(),
        }
    }

    /// The `fullUrl` (and reference target) of a resource of `resource_type` with `id`.
    #[must_use]
    pub fn url_for(&self, resource_type: &str, id: &str) -> String {
        urn_for(&self.seed, resource_type, id)
    }

    /// Add a resource (a JSON object with `resourceType` and `id`). The entry's `fullUrl` comes
    /// from [`Bundle::url_for`]; a resource without an `id` is keyed by its position instead.
    pub fn push(&mut self, resource: Value) {
        let resource_type = resource["resourceType"].as_str().unwrap_or("Unknown");
        let id = resource["id"].as_str().map_or_else(
            || format!("entry-{}", self.entries.len() + 1),
            str::to_owned,
        );
        let url = self.url_for(resource_type, &id);
        self.entries.push((url, resource));
    }

    /// Number of resources in the bundle.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if the bundle holds no resources.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The bundle as a FHIR JSON value.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let entry: Vec<Value> = self
            .entries
            .iter()
            .map(|(url, resource)| json!({ "fullUrl": url, "resource": resource }))
            .collect();
        json!({
            "resourceType": "Bundle",
            "type": "collection",
            "entry": entry,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bundle_serializes() {
        let json = Bundle::new().to_json();
        assert_eq!(json["resourceType"], "Bundle");
        assert_eq!(json["type"], "collection");
        assert_eq!(json["entry"], json!([]));
    }

    #[test]
    fn resources_are_wrapped_in_entries() {
        let mut bundle = Bundle::new();
        bundle.push(json!({ "resourceType": "Patient" }));
        assert_eq!(bundle.len(), 1);
        assert_eq!(
            bundle.to_json()["entry"][0]["resource"]["resourceType"],
            "Patient"
        );
        assert!(bundle.to_json()["entry"][0]["fullUrl"].is_string());
    }

    #[test]
    fn entries_get_deterministic_urn_uuid_full_urls() {
        let mut a = Bundle::with_namespace_seed("doc-1");
        a.push(json!({ "resourceType": "Patient", "id": "patient-1" }));
        let url = a.to_json()["entry"][0]["fullUrl"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(url.starts_with("urn:uuid:"));
        assert_eq!(url, a.url_for("Patient", "patient-1"));

        let mut again = Bundle::with_namespace_seed("doc-1");
        again.push(json!({ "resourceType": "Patient", "id": "patient-1" }));
        assert_eq!(again.to_json(), a.to_json());
    }

    #[test]
    fn different_documents_and_resources_get_different_urls() {
        let a = Bundle::with_namespace_seed("doc-1");
        let b = Bundle::with_namespace_seed("doc-2");
        assert_ne!(
            a.url_for("Patient", "patient-1"),
            b.url_for("Patient", "patient-1")
        );
        assert_ne!(
            a.url_for("Patient", "patient-1"),
            a.url_for("Condition", "patient-1")
        );
    }
}
