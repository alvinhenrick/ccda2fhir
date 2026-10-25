//! State shared by the resource mappers for one document.

use std::collections::BTreeMap;

use ccda_terminology::Terminology;
use serde_json::Value;

use crate::{Report, PATIENT_ID};

/// What a mapper needs besides the model: terminology, the report, the patient reference and a
/// deterministic id source.
pub struct Context<'a> {
    pub terminology: &'a dyn Terminology,
    pub report: &'a mut Report,
    /// Reference every patient-owned resource points at, e.g. `Patient/patient-1`.
    pub patient: String,
    seed: String,
    counters: BTreeMap<&'static str, u32>,
    /// Practitioners and organizations already created, by identity key, so one person who
    /// authored many entries becomes one resource.
    pub(crate) practitioners: BTreeMap<String, String>,
    pub(crate) organizations: BTreeMap<String, String>,
    /// Locations and specimens already created, by identity key.
    pub(crate) locations: BTreeMap<String, String>,
    pub(crate) specimens: BTreeMap<String, String>,
    /// Device ids by UDI, so a procedure can point at a device that is mapped later.
    pub(crate) devices: BTreeMap<String, String>,
    /// Supporting resources (Practitioner, Organization, Provenance) created by mappers.
    extra: Vec<Value>,
}

impl<'a> Context<'a> {
    #[must_use]
    /// `seed` namespaces the `urn:uuid:` references (the converter uses the document id) and
    /// must match the seed of the bundle the resources are added to.
    pub fn new(
        terminology: &'a dyn Terminology,
        report: &'a mut Report,
        seed: impl Into<String>,
    ) -> Self {
        let seed = seed.into();
        Self {
            terminology,
            report,
            patient: fhir_out::urn_for(&seed, "Patient", PATIENT_ID),
            seed,
            counters: BTreeMap::new(),
            practitioners: BTreeMap::new(),
            organizations: BTreeMap::new(),
            locations: BTreeMap::new(),
            specimens: BTreeMap::new(),
            devices: BTreeMap::new(),
            extra: Vec::new(),
        }
    }

    /// The reference (`urn:uuid:...`) to a resource, as used in `fullUrl` and `reference`.
    #[must_use]
    pub fn reference(&self, resource_type: &str, id: &str) -> String {
        fhir_out::urn_for(&self.seed, resource_type, id)
    }

    /// Add a supporting resource to be emitted after the clinical resources.
    pub(crate) fn push_extra(&mut self, resource: Value) {
        self.extra.push(resource);
    }

    /// Take the supporting resources created so far, in creation order.
    pub fn take_extra(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.extra)
    }

    /// The next id for a resource type: `condition-1`, `condition-2`, ...
    ///
    /// Ids depend only on document order, so converting the same document twice gives the same
    /// ids. (A content-derived scheme can replace this later without changing mappers.)
    pub fn next_id(&mut self, prefix: &'static str) -> String {
        let n = self.counters.entry(prefix).or_insert(0);
        *n += 1;
        format!("{prefix}-{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccda_terminology::BuiltinTerminology;

    #[test]
    fn ids_count_per_prefix_in_order() {
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        assert_eq!(ctx.next_id("condition"), "condition-1");
        assert_eq!(ctx.next_id("condition"), "condition-2");
        assert_eq!(ctx.next_id("observation"), "observation-1");
    }
}
