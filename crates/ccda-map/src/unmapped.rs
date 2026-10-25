//! Entries that no mapper handles.
//!
//! A converter that claims to be exhaustive must not drop data silently. Every top-level entry
//! whose template no mapper handles gets one report entry naming the template (an OID, which is
//! document structure, not patient data).

use ccda_model::ClinicalDocument;

use crate::Context;

/// Entry-level templates that a mapper handles (see the mapper modules).
const HANDLED: &[&str] = &[
    "2.16.840.1.113883.10.20.22.4.1",   // Result Organizer
    "2.16.840.1.113883.10.20.22.4.2",   // Result Observation
    "2.16.840.1.113883.10.20.22.4.3",   // Problem Concern Act
    "2.16.840.1.113883.10.20.22.4.12",  // Procedure Activity Act
    "2.16.840.1.113883.10.20.22.4.14",  // Procedure Activity Procedure
    "2.16.840.1.113883.10.20.22.4.16",  // Medication Activity
    "2.16.840.1.113883.10.20.22.4.26",  // Vital Signs Organizer
    "2.16.840.1.113883.10.20.22.4.27",  // Vital Sign Observation
    "2.16.840.1.113883.10.20.22.4.30",  // Allergy Concern Act
    "2.16.840.1.113883.10.20.22.4.38",  // Social History Observation
    "2.16.840.1.113883.10.20.22.4.39",  // Planned Act
    "2.16.840.1.113883.10.20.22.4.40",  // Planned Encounter
    "2.16.840.1.113883.10.20.22.4.41",  // Planned Procedure
    "2.16.840.1.113883.10.20.22.4.44",  // Planned Observation
    "2.16.840.1.113883.10.20.22.4.49",  // Encounter Activity
    "2.16.840.1.113883.10.20.22.4.52",  // Immunization Activity
    "2.16.840.1.113883.10.20.22.4.60",  // Coverage Activity
    "2.16.840.1.113883.10.20.22.4.61",  // Policy Activity
    "2.16.840.1.113883.10.20.22.4.78",  // Smoking Status
    "2.16.840.1.113883.10.20.22.4.85",  // Tobacco Use
    "2.16.840.1.113883.10.20.22.4.121", // Goal Observation
    "2.16.840.1.113883.10.20.22.4.132", // Health Concern Act
    "2.16.840.1.113883.10.20.22.4.200", // Birth Sex (Patient)
    "2.16.840.1.113883.10.20.22.4.202", // Note Activity
    "2.16.840.1.113883.10.20.22.4.500", // Care Team Organizer
    "2.16.840.1.113883.10.20.15.3.8",   // Pregnancy Observation
    "2.16.840.1.113883.10.20.34.3.45",  // Gender Identity (Patient)
];

/// Report every top-level entry that no mapper handles.
pub fn report_unmapped(doc: &ClinicalDocument, ctx: &mut Context) {
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.template_ids
                .iter()
                .any(|t| HANDLED.contains(&t.root.as_str()))
            {
                continue;
            }
            // The root goes in the message only if it is an OID; anything else could be document text.
            let message = match s.template_ids.first() {
                Some(t) if ccda_terminology::is_oid(&t.root) => {
                    format!("an entry with template {} was not mapped", t.root)
                }
                Some(_) => "an entry with an unrecognized templateId was not mapped".to_owned(),
                None => "an entry with no templateId was not mapped".to_owned(),
            };
            ctx.report.info("entry.unmapped", message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(entries: &str) -> Vec<String> {
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><component><structuredBody><component><section>{entries}</section></component></structuredBody></component></ClinicalDocument>"#
        );
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        report_unmapped(&d, &mut ctx);
        report
            .issues
            .iter()
            .filter(|i| i.code == "entry.unmapped")
            .map(|i| i.message.clone())
            .collect()
    }

    #[test]
    fn handled_templates_are_not_reported() {
        let out =
            run(r#"<entry><act><templateId root="2.16.840.1.113883.10.20.22.4.3"/></act></entry>"#);
        assert!(out.is_empty());
    }

    #[test]
    fn unhandled_template_is_reported_by_oid() {
        let out = run(
            r#"<entry><act><templateId root="2.16.840.1.113883.10.20.22.4.66"/></act></entry>"#,
        );
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("2.16.840.1.113883.10.20.22.4.66"));
    }

    #[test]
    fn a_template_root_that_is_not_an_oid_is_never_echoed() {
        let out = run(r#"<entry><act><templateId root="Jane Doe 555-1234"/></act></entry>"#);
        assert_eq!(out.len(), 1);
        assert!(!out[0].contains("Jane"));
        let out = run(r#"<entry><act/></entry>"#);
        assert!(out[0].contains("no templateId"));
    }
}
