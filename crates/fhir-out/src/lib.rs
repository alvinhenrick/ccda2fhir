//! FHIR R4 output building blocks.
//!
//! Resources are plain JSON values for now. That keeps the early phases fast and lets us emit
//! profile-specific elements and extensions freely. Typed resources can replace this later without
//! touching parsing or mapping, because FHIR knowledge stays inside this crate.

use serde_json::{json, Value};

/// A FHIR R4 Bundle of type `collection`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bundle {
    entries: Vec<Value>,
}

impl Bundle {
    /// An empty bundle.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a resource (a JSON object with a `resourceType`).
    pub fn push(&mut self, resource: Value) {
        self.entries.push(resource);
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
            .map(|resource| json!({ "resource": resource }))
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
    }
}
