//! Mapping from the typed C-CDA model to FHIR R4 resources.
//!
//! Every mapper takes the model plus a [`Report`] and returns resources. Anything lossy,
//! unexpected or unsupported becomes an [`Issue`] in the report instead of an error or a silent
//! drop. Issue messages never contain document values, so reports are safe to log.
//!
//! The shared building blocks (timestamps, `nullFlavor`, identifiers, coded values) follow the
//! IG's general mapping guidance and are reused by every resource mapper.

mod allergy;
mod author;
mod care_plan;
mod care_team;
mod coding;
mod composition;
mod context;
mod coverage;
mod device;
mod encounter;
mod goal;
mod health_concern;
mod identifier;
mod immunization;
mod location;
mod medication;
mod note;
mod nullflavor;
mod observation;
mod patient;
mod problem;
mod procedure;
mod profile;
mod quantity;
mod report;
mod results;
mod service_request;
mod social;
mod specimen;
mod timestamp;
mod unmapped;
mod util;
mod vitals;

pub use allergy::map_allergies;
pub use care_plan::map_care_plans;
pub use care_team::map_care_teams;
pub use coding::map_codeable_concept;
pub use composition::map_composition;
pub use context::Context;
pub use coverage::map_coverage;
pub use device::map_devices;
pub use encounter::map_encounters;
pub use goal::map_goals;
pub use health_concern::map_health_concerns;
pub use identifier::map_identifier;
pub use immunization::map_immunizations;
pub use medication::map_medications;
pub use note::map_notes;
pub use nullflavor::{
    data_absent_reason_code, data_absent_reason_extension, DATA_ABSENT_REASON_URL,
};
pub use patient::{map_patient, PATIENT_ID};
pub use problem::map_problems;
pub use procedure::map_procedures;
pub use profile::Profiles;
pub use quantity::{decimal, map_age, map_quantity};
pub use report::{Issue, Report, Severity};
pub use results::map_results;
pub use service_request::map_service_requests;
pub use social::map_social_history;
pub use timestamp::{
    hl7_date_to_fhir_date, hl7_ts_to_fhir_datetime, map_datetime, Converted, InvalidTimestamp, Loss,
};
pub use unmapped::report_unmapped;
pub use vitals::map_vitals;
