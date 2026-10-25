//! HL7 v3 timestamps to FHIR `date`.

/// Convert an HL7 v3 `TS` value to a FHIR `date`, keeping the precision it has.
///
/// `19700101120000-0500` becomes `1970-01-01`, `197001` becomes `1970-01`, `1970` becomes `1970`.
/// Returns `None` if the value is too short, not numeric, or has an impossible month or day.
/// The time and offset are dropped on purpose: a `date` has neither.
#[must_use]
pub fn hl7_date_to_fhir_date(value: &str) -> Option<String> {
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    let year = digits.get(0..4)?;
    let Some(month) = digits.get(4..6) else {
        return Some(year.to_owned());
    };
    if !(1..=12).contains(&month.parse::<u8>().ok()?) {
        return None;
    }
    let Some(day) = digits.get(6..8) else {
        return Some(format!("{year}-{month}"));
    };
    if !(1..=31).contains(&day.parse::<u8>().ok()?) {
        return None;
    }
    Some(format!("{year}-{month}-{day}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_available_precision() {
        assert_eq!(hl7_date_to_fhir_date("1970").as_deref(), Some("1970"));
        assert_eq!(hl7_date_to_fhir_date("197001").as_deref(), Some("1970-01"));
        assert_eq!(
            hl7_date_to_fhir_date("19700102").as_deref(),
            Some("1970-01-02")
        );
    }

    #[test]
    fn drops_time_and_offset() {
        assert_eq!(
            hl7_date_to_fhir_date("19700102120000-0500").as_deref(),
            Some("1970-01-02")
        );
    }

    #[test]
    fn rejects_bad_values() {
        assert_eq!(hl7_date_to_fhir_date(""), None);
        assert_eq!(hl7_date_to_fhir_date("19"), None);
        assert_eq!(hl7_date_to_fhir_date("abcd"), None);
        assert_eq!(hl7_date_to_fhir_date("19701301"), None);
        assert_eq!(hl7_date_to_fhir_date("19700132"), None);
    }
}
