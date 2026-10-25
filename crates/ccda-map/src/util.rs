//! Small helpers shared by the resource mappers.

use ccda_model::{Statement, TimePoint};
use serde_json::{json, Value};

pub(crate) const LOINC: &str = "2.16.840.1.113883.6.1";
pub(crate) const SNOMED: &str = "2.16.840.1.113883.6.96";

/// True if the statement's code is `code` in the code system `oid`.
pub(crate) fn is_coded(statement: &Statement, code: &str, oid: &str) -> bool {
    statement
        .code
        .as_ref()
        .is_some_and(|c| c.code.as_deref() == Some(code) && c.code_system.as_deref() == Some(oid))
}

/// A `CodeableConcept` with one coding.
pub(crate) fn concept(system: &str, code: &str) -> Value {
    json!({ "coding": [{ "system": system, "code": code }] })
}

/// The start of an `effectiveTime`: `low`, or the point `value` when there is no interval.
pub(crate) fn start_of(statement: &Statement) -> Option<TimePoint> {
    let time = statement.effective_time.as_ref()?;
    time.low.clone().or_else(|| {
        time.value.clone().map(|value| TimePoint {
            value: Some(value),
            null_flavor: None,
        })
    })
}

/// Statements nested anywhere directly under `statement` (any link type).
pub(crate) fn nested(statement: &Statement) -> impl Iterator<Item = &Statement> {
    statement.related.iter().map(|r| &r.statement)
}

/// Intervention Act (4.131) and Planned Intervention Act (4.146): wrappers around the activities
/// that are the actual interventions.
const INTERVENTION_ACTS: &[&str] = &[
    "2.16.840.1.113883.10.20.22.4.131",
    "2.16.840.1.113883.10.20.22.4.146",
];

/// The statements of a section that mappers treat as top-level entries: each entry, except that
/// an intervention wrapper is replaced by the activities inside it.
pub(crate) fn activities(section: &ccda_model::Section) -> Vec<&Statement> {
    fn walk<'a>(s: &'a Statement, out: &mut Vec<&'a Statement>) {
        if INTERVENTION_ACTS.iter().any(|t| s.has_template(t)) {
            for r in &s.related {
                walk(&r.statement, out);
            }
        } else {
            out.push(s);
        }
    }
    let mut out = Vec::new();
    for entry in &section.entries {
        walk(&entry.statement, &mut out);
    }
    out
}

/// An XHTML narrative `div` holding `text` as one escaped paragraph.
pub(crate) fn narrative_div(text: &str) -> String {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!("<div xmlns=\"http://www.w3.org/1999/xhtml\"><p>{escaped}</p></div>")
}
