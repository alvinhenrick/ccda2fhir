//! Mapping from the typed C-CDA model to FHIR R4 resources.
//!
//! Every mapper takes the model plus a [`Report`] and returns resources. Anything lossy,
//! unexpected or unsupported becomes an [`Issue`] in the report instead of an error or a silent
//! drop. Issue messages never contain document values, so reports are safe to log.

mod dates;
mod patient;
mod report;

pub use dates::hl7_date_to_fhir_date;
pub use patient::map_patient;
pub use report::{Issue, Report, Severity};
