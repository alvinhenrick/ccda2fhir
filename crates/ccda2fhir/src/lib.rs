//! Convert C-CDA documents to FHIR R4 bundles.
//!
//! ```
//! let xml = br#"<ClinicalDocument xmlns="urn:hl7-org:v3"/>"#;
//! let conversion = ccda2fhir::convert(xml, &ccda2fhir::Options::default()).unwrap();
//! assert_eq!(conversion.bundle.to_json()["resourceType"], "Bundle");
//! ```
//!
//! The library is pure: bytes in, a [`Conversion`] out. It performs no I/O and keeps no state.

use ccda_map::{
    add_generated_text, link_encounters, link_managing_organization, map_allergies,
    map_assessments, map_care_plans, map_care_teams, map_composition, map_coverage, map_devices,
    map_encompassing_encounter, map_encounters, map_family_history, map_goals, map_health_concerns,
    map_immunizations, map_medications, map_notes, map_patient, map_problems, map_procedures,
    map_results, map_service_requests, map_social_history, map_unstructured, map_vitals,
    report_unmapped, Context, Profiles, Report,
};
use ccda_model::Limits;
use ccda_terminology::BuiltinTerminology;
use fhir_out::Bundle;
use thiserror::Error;

/// Settings for a conversion.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Input safety limits applied while parsing.
    pub limits: Limits,
    /// Profiles to declare in `meta.profile`. US Core 6.1.0 by default.
    pub profiles: Profiles,
}

/// The result of a successful conversion.
#[derive(Debug, Clone)]
pub struct Conversion {
    /// The FHIR bundle.
    pub bundle: Bundle,
    /// Everything lossy or unexpected that happened on the way.
    pub report: Report,
}

/// Failures that stop a conversion. Data problems are reported in [`Report`] instead.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConvertError {
    /// The input could not be parsed as a C-CDA document.
    #[error("could not parse the C-CDA document: {0}")]
    Parse(#[from] ccda_model::ParseError),
}

/// Convert a C-CDA document to a FHIR R4 bundle.
///
/// # Errors
///
/// Returns [`ConvertError::Parse`] if the input is too large, not UTF-8, not well-formed XML, or
/// not a C-CDA `ClinicalDocument`.
pub fn convert(xml: &[u8], options: &Options) -> Result<Conversion, ConvertError> {
    let document = ccda_model::parse_with_limits(xml, &options.limits)?;

    let mut report = Report::default();
    // Urls are namespaced by the document id, so bundles from different documents never collide.
    let seed = document
        .id
        .as_ref()
        .map(|id| {
            format!(
                "{}|{}",
                id.root.as_deref().unwrap_or_default(),
                id.extension.as_deref().unwrap_or_default()
            )
        })
        .unwrap_or_default();
    let mut bundle = Bundle::with_namespace_seed(&seed);
    let profiles = options.profiles;
    let mut ctx = Context::new(&BuiltinTerminology, &mut report, &seed);
    let mut patient = map_patient(&document, &BuiltinTerminology, ctx.report);
    link_managing_organization(&document, &mut ctx, &mut patient);
    add_generated_text(&mut patient);
    profiles.apply(&mut patient);
    bundle.push(patient);

    let mut resources: Vec<serde_json::Value> = map_problems(&document, &mut ctx)
        .into_iter()
        .chain(map_allergies(&document, &mut ctx))
        .chain(map_immunizations(&document, &mut ctx))
        .chain(map_medications(&document, &mut ctx))
        .chain(map_vitals(&document, &mut ctx))
        .chain(map_results(&document, &mut ctx))
        .chain(map_procedures(&document, &mut ctx))
        .chain(map_encounters(&document, &mut ctx))
        .chain(map_family_history(&document, &mut ctx))
        .chain(map_social_history(&document, &mut ctx))
        .chain(map_notes(&document, &mut ctx))
        .chain(map_devices(&document, &mut ctx))
        .chain(goals_and_plans(&document, &mut ctx))
        .chain(map_care_teams(&document, &mut ctx))
        .chain(map_coverage(&document, &mut ctx))
        .chain(map_service_requests(&document, &mut ctx))
        .chain(map_assessments(&document, &mut ctx))
        .chain(map_unstructured(&document, &mut ctx))
        .collect();
    map_encompassing_encounter(&document, &mut ctx, &mut resources);
    // Health concerns mark or add Conditions, so they run after everything else is mapped.
    let concerns = map_health_concerns(&document, &mut ctx, &mut resources);
    resources.extend(concerns);
    link_encounters(&document, &mut ctx, &mut resources);
    report_unmapped(&document, &mut ctx);
    // The Composition lists the resources above, so it is built last and placed right after the
    // patient.
    let mut composition = map_composition(&document, &mut ctx, &resources);
    add_generated_text(&mut composition);
    profiles.apply(&mut composition);
    bundle.push(composition);
    for mut resource in resources {
        add_generated_text(&mut resource);
        profiles.apply(&mut resource);
        bundle.push(resource);
    }
    for mut resource in ctx.take_extra() {
        add_generated_text(&mut resource);
        profiles.apply(&mut resource);
        bundle.push(resource);
    }

    Ok(Conversion { bundle, report })
}

/// Goals, then the care plans that point at them.
fn goals_and_plans(
    document: &ccda_model::ClinicalDocument,
    ctx: &mut Context,
) -> Vec<serde_json::Value> {
    let mut out = map_goals(document, ctx);
    let plans = map_care_plans(document, ctx, &out);
    out.extend(plans);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SAMPLE: &str = r#"<ClinicalDocument xmlns="urn:hl7-org:v3">
  <recordTarget><patientRole>
    <id root="2.16.840.1.113883.4.1" extension="000-00-0000"/>
    <patient>
    <name><given>Alice</given><family>Example</family></name>
    <administrativeGenderCode code="F"/>
    <birthTime value="19700101"/>
  </patient></patientRole></recordTarget>
</ClinicalDocument>"#;

    #[test]
    fn converts_header_patient_to_a_bundle() {
        let conversion = convert(SAMPLE.as_bytes(), &Options::default()).unwrap();
        let json = conversion.bundle.to_json();
        assert_eq!(json["entry"][0]["resource"]["resourceType"], "Patient");
        assert_eq!(json["entry"][0]["resource"]["birthDate"], "1970-01-01");
        assert_eq!(
            json["entry"][0]["resource"]["identifier"][0]["system"],
            "http://hl7.org/fhir/sid/us-ssn"
        );
        // The sample has no author, so the only thing reported is that no Composition was built.
        assert!(conversion
            .report
            .issues
            .iter()
            .all(|i| i.code.starts_with("composition.")));
    }

    #[test]
    fn parse_failures_are_errors_without_document_content() {
        let err = convert(b"<Secret-Name/>", &Options::default()).unwrap_err();
        assert!(matches!(err, ConvertError::Parse(_)));
        assert!(!err.to_string().contains("Secret"));
    }

    #[test]
    fn output_is_deterministic() {
        let a = convert(SAMPLE.as_bytes(), &Options::default()).unwrap();
        let b = convert(SAMPLE.as_bytes(), &Options::default()).unwrap();
        assert_eq!(a.bundle.to_json(), b.bundle.to_json());
        assert_eq!(a.bundle.to_json()["type"], json!("collection"));
    }
}
