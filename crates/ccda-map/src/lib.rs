//! Mapping from the typed C-CDA model to FHIR R4 resources.
//!
//! Every mapper takes the model plus a [`Report`] and returns resources. Anything lossy,
//! unexpected or unsupported becomes an [`Issue`] in the report instead of an error or a silent
//! drop. Issue messages never contain document values, so reports are safe to log.
//!
//! The shared building blocks (timestamps, `nullFlavor`, identifiers, coded values) follow the
//! IG's general mapping guidance and are reused by every resource mapper.

mod coding;
mod identifier;
mod nullflavor;
mod patient;
mod report;
mod timestamp;

pub use coding::map_codeable_concept;
pub use identifier::map_identifier;
pub use nullflavor::{
    data_absent_reason_code, data_absent_reason_extension, DATA_ABSENT_REASON_URL,
};
pub use patient::map_patient;
pub use report::{Issue, Report, Severity};
pub use timestamp::{
    hl7_date_to_fhir_date, hl7_ts_to_fhir_datetime, map_datetime, Converted, InvalidTimestamp, Loss,
};
