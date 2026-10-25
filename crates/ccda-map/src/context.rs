//! State shared by the resource mappers for one document.

use std::collections::BTreeMap;

use ccda_terminology::Terminology;

use crate::Report;

/// What a mapper needs besides the model: terminology, the report, the patient reference and a
/// deterministic id source.
pub struct Context<'a> {
    pub terminology: &'a dyn Terminology,
    pub report: &'a mut Report,
    /// Reference every patient-owned resource points at, e.g. `Patient/patient-1`.
    pub patient: String,
    counters: BTreeMap<&'static str, u32>,
}

impl<'a> Context<'a> {
    #[must_use]
    pub fn new(
        terminology: &'a dyn Terminology,
        report: &'a mut Report,
        patient: impl Into<String>,
    ) -> Self {
        Self {
            terminology,
            report,
            patient: patient.into(),
            counters: BTreeMap::new(),
        }
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
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "Patient/patient-1");
        assert_eq!(ctx.next_id("condition"), "condition-1");
        assert_eq!(ctx.next_id("condition"), "condition-2");
        assert_eq!(ctx.next_id("observation"), "observation-1");
    }
}
