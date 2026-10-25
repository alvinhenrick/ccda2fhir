//! C-CDA `PQ` to FHIR `Quantity` / `Age`.
//!
//! FHIR decimals keep their written precision (`5.40` is not `5.4`), so numbers are copied from
//! the source text, never round-tripped through a float. That relies on `serde_json`'s
//! `arbitrary_precision` feature.

use ccda_model::Quantity;
use serde_json::{json, Map, Number, Value};

use crate::Report;

/// UCUM, the code system FHIR expects for units.
const UCUM: &str = "http://unitsofmeasure.org";

/// True for the FHIR/XML Schema decimal grammar: optional sign, digits, optional fraction.
/// Exponents are not allowed in FHIR decimals.
fn is_decimal(text: &str) -> bool {
    let body = text.strip_prefix(['-', '+']).unwrap_or(text);
    let (int, frac) = body
        .split_once('.')
        .map_or((body, None), |(i, f)| (i, Some(f)));
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    digits(int) && frac.is_none_or(digits)
}

/// A FHIR decimal number with the source's precision, or `None` if `text` is not a decimal.
#[must_use]
pub fn decimal(text: &str) -> Option<Number> {
    let text = text.strip_prefix('+').unwrap_or(text);
    if !is_decimal(text) {
        return None;
    }
    serde_json::from_str(text).ok()
}

/// Map a physical quantity to a FHIR `Quantity`. Returns `None` (with a warning) when the value
/// is missing or not a decimal.
pub fn map_quantity(quantity: &Quantity, report: &mut Report) -> Option<Value> {
    let raw = quantity.value.as_deref()?;
    let Some(number) = decimal(raw) else {
        report.warn(
            "quantity.value.invalid",
            "a quantity value is not a decimal number and was omitted",
        );
        return None;
    };
    let mut out = Map::new();
    out.insert("value".into(), Value::Number(number));
    if let Some(unit) = quantity.unit.as_deref().filter(|u| *u != "1") {
        out.insert("unit".into(), json!(unit));
        out.insert("system".into(), json!(UCUM));
        out.insert("code".into(), json!(unit));
    }
    Some(Value::Object(out))
}

/// Map a physical quantity to a FHIR `Age`. FHIR restricts ages to the UCUM units
/// `a`, `mo`, `wk`, `d`, `h` and `min`; anything else is reported and dropped.
pub fn map_age(quantity: &Quantity, report: &mut Report) -> Option<Value> {
    let unit = quantity.unit.as_deref().unwrap_or_default();
    if !matches!(unit, "a" | "mo" | "wk" | "d" | "h" | "min") {
        report.warn(
            "quantity.age.unit",
            "an age quantity does not use a UCUM time unit FHIR allows for Age and was omitted",
        );
        return None;
    }
    map_quantity(quantity, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(value: &str, unit: Option<&str>) -> Quantity {
        Quantity {
            value: Some(value.into()),
            unit: unit.map(Into::into),
            null_flavor: None,
        }
    }

    #[test]
    fn keeps_decimal_precision() {
        let mut report = Report::default();
        let v = map_quantity(&q("5.40", Some("mg/dL")), &mut report).unwrap();
        assert_eq!(v["unit"], "mg/dL");
        assert_eq!(v["system"], "http://unitsofmeasure.org");
        assert_eq!(v["code"], "mg/dL");
        // Written precision survives: 5.40, not 5.4.
        assert_eq!(v["value"].to_string(), "5.40");
        assert!(report.issues.is_empty());
    }

    #[test]
    fn unity_unit_means_no_unit() {
        let mut report = Report::default();
        let v = map_quantity(&q("3", Some("1")), &mut report).unwrap();
        assert_eq!(v.to_string(), r#"{"value":3}"#);
    }

    #[test]
    fn rejects_non_decimals_without_echoing_them() {
        for bad in ["", "abc", "1e3", "1.", ".5", "--1", "1,5", "NaN", "1.2.3"] {
            assert_eq!(decimal(bad), None, "{bad}");
        }
        let mut report = Report::default();
        assert_eq!(map_quantity(&q("12x", None), &mut report), None);
        assert_eq!(report.issues[0].code, "quantity.value.invalid");
        assert!(!report.issues[0].message.contains("12x"));
    }

    #[test]
    fn accepts_signed_decimals() {
        assert!(decimal("-0.5").is_some());
        assert!(decimal("+7").is_some());
        assert_eq!(decimal("+7").unwrap().to_string(), "7");
    }

    #[test]
    fn missing_value_is_none_without_a_warning() {
        let mut report = Report::default();
        let none = Quantity::default();
        assert_eq!(map_quantity(&none, &mut report), None);
        assert!(report.issues.is_empty());
    }

    #[test]
    fn age_requires_an_allowed_unit() {
        let mut report = Report::default();
        assert!(map_age(&q("45", Some("a")), &mut report).is_some());
        assert!(map_age(&q("45", Some("kg")), &mut report).is_none());
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].code, "quantity.age.unit");
    }
}
