//! Social determinants of health (SDOH) tagging.
//!
//! C-CDA has no SDOH flag, so the category is derived from the code, using only lists that are
//! unambiguous: ICD-10-CM Z55 to Z65 ("persons with potential health hazards related to
//! socioeconomic and psychosocial circumstances") for Conditions, and a short list of LOINC
//! SDOH screening panels and items for Observations. Every tag is reported.

use serde_json::{json, Value};

use crate::Context;

const US_CORE_CATEGORY: &str = "http://hl7.org/fhir/us/core/CodeSystem/us-core-category";
const ICD10CM: &str = "http://hl7.org/fhir/sid/icd-10-cm";
const LOINC: &str = "http://loinc.org";

/// LOINC SDOH screening tools and items: PRAPARE, AHC-HRSN, Hunger Vital Sign (panel and
/// items), housing status.
const SDOH_LOINC: &[&str] = &[
    "93025-5", "96777-8", "88121-9", "88122-7", "88123-5", "71802-3",
];

fn is_sdoh_icd10(code: &str) -> bool {
    let b = code.as_bytes();
    b.len() >= 3
        && b[0] == b'Z'
        && matches!((b[1], b[2]), (b'5', b'5'..=b'9') | (b'6', b'0'..=b'5'))
}

fn has_coding(resource: &Value, test: impl Fn(&str, &str) -> bool) -> bool {
    resource["code"]["coding"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|c| {
            test(
                c["system"].as_str().unwrap_or_default(),
                c["code"].as_str().unwrap_or_default(),
            )
        })
}

fn tag(resource: &mut Value) {
    let coding = json!({ "system": US_CORE_CATEGORY, "code": "sdoh", "display": "SDOH" });
    match resource["category"].as_array_mut() {
        Some(categories) => categories.push(json!({ "coding": [coding] })),
        None => resource["category"] = json!([{ "coding": [coding] }]),
    }
}

/// Add the `sdoh` category to Conditions and Observations that are SDOH by code.
pub fn tag_sdoh(ctx: &mut Context, resources: &mut [Value]) {
    for r in resources.iter_mut() {
        let sdoh = match r["resourceType"].as_str() {
            Some("Condition") => has_coding(r, |s, c| s == ICD10CM && is_sdoh_icd10(c)),
            Some("Observation") => has_coding(r, |s, c| s == LOINC && SDOH_LOINC.contains(&c)),
            _ => false,
        };
        if sdoh {
            tag(r);
            ctx.report.info(
                "sdoh.category.derived",
                "a resource was given the sdoh category because its code is on the SDOH code list",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(mut r: Value) -> (Value, Report) {
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        tag_sdoh(&mut ctx, std::slice::from_mut(&mut r));
        (r, report)
    }

    #[test]
    fn z_codes_55_to_65_are_sdoh() {
        for (code, expected) in [
            ("Z59.48", true),
            ("Z55.0", true),
            ("Z65.9", true),
            ("Z54.0", false),
            ("Z66", false),
            ("I10", false),
        ] {
            let (r, _) = run(json!({
                "resourceType": "Condition",
                "category": [{ "coding": [{ "code": "problem-list-item" }] }],
                "code": { "coding": [{ "system": ICD10CM, "code": code }] },
            }));
            assert_eq!(
                r["category"].as_array().unwrap().len() == 2,
                expected,
                "{code}"
            );
        }
    }

    #[test]
    fn screening_observation_gets_a_category_even_without_one() {
        let (r, rep) = run(json!({
            "resourceType": "Observation",
            "code": { "coding": [{ "system": LOINC, "code": "93025-5" }] },
        }));
        assert_eq!(r["category"][0]["coding"][0]["code"], "sdoh");
        assert!(rep.issues.iter().any(|i| i.code == "sdoh.category.derived"));
    }
}
