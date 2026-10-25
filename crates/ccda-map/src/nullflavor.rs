//! HL7 `nullFlavor` to the FHIR data-absent-reason extension.
//!
//! The table is the IG's own ConceptMap "NullFlavor to Data Absent Reason"
//! (`input/maps/ConceptMap-CF-NullFlavorDataAbsentReason.xml` in HL7/ccda-on-fhir).

use serde_json::{json, Value};

use crate::Report;

/// URL of the FHIR data-absent-reason extension.
pub const DATA_ABSENT_REASON_URL: &str =
    "http://hl7.org/fhir/StructureDefinition/data-absent-reason";

/// The data-absent-reason code for a `nullFlavor`, per the IG. `None` if the IG has no mapping
/// (for example `INV`, `DER` or `UNC`).
#[must_use]
pub fn data_absent_reason_code(null_flavor: &str) -> Option<&'static str> {
    Some(match null_flavor {
        "NI" | "UNK" | "NP" => "unknown",
        "OTH" | "TRC" => "unsupported",
        "NINF" => "negative-infinity",
        "PINF" => "positive-infinity",
        "MSK" => "masked",
        "NA" => "not-applicable",
        "ASKU" => "asked-unknown",
        "NAV" => "temp-unknown",
        "NASK" => "not-asked",
        _ => return None,
    })
}

/// A data-absent-reason extension for a `nullFlavor`.
///
/// If the IG has no mapping, the reason is `unknown` and a warning is recorded.
pub fn data_absent_reason_extension(null_flavor: &str, report: &mut Report) -> Value {
    let code = data_absent_reason_code(null_flavor).unwrap_or_else(|| {
        report.warn(
            "nullflavor.unmapped",
            "a nullFlavor has no mapping in the IG; data-absent-reason was set to unknown",
        );
        "unknown"
    });
    json!({ "url": DATA_ABSENT_REASON_URL, "valueCode": code })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_every_nullflavor_in_the_ig() {
        let expected = [
            ("NI", "unknown"),
            ("OTH", "unsupported"),
            ("NINF", "negative-infinity"),
            ("PINF", "positive-infinity"),
            ("MSK", "masked"),
            ("NA", "not-applicable"),
            ("UNK", "unknown"),
            ("ASKU", "asked-unknown"),
            ("NAV", "temp-unknown"),
            ("NASK", "not-asked"),
            ("TRC", "unsupported"),
            ("NP", "unknown"),
        ];
        for (nf, code) in expected {
            assert_eq!(data_absent_reason_code(nf), Some(code), "{nf}");
        }
    }

    #[test]
    fn unmapped_nullflavor_becomes_unknown_with_a_warning() {
        let mut report = Report::default();
        let ext = data_absent_reason_extension("INV", &mut report);
        assert_eq!(ext["valueCode"], "unknown");
        assert_eq!(ext["url"], DATA_ABSENT_REASON_URL);
        assert_eq!(report.issues[0].code, "nullflavor.unmapped");
        assert!(!report.issues[0].message.contains("INV"));
    }

    #[test]
    fn mapped_nullflavor_records_nothing() {
        let mut report = Report::default();
        data_absent_reason_extension("NASK", &mut report);
        assert!(report.issues.is_empty());
    }
}
