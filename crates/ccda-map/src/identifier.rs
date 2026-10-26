//! C-CDA `id` to FHIR `Identifier`, following the IG's "CDA id to FHIR identifier" table.

use ccda_model::Identifier;
use ccda_terminology::{is_oid, Terminology};
use serde_json::{json, Map, Value};

use crate::nullflavor::data_absent_reason_extension;
use crate::Report;

/// The OID for `urn:ietf:rfc:3986` (URI). A root with this value carries a URL or URN in its
/// extension, which is split into `system` and `value`.
const URI_OID: &str = "2.16.840.1.113883.4.873";

/// FHIR `identifier.system` used when an id has only a root, which is its own value.
const URI_SYSTEM: &str = "urn:ietf:rfc:3986";

fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// Map a C-CDA `id` to a FHIR `Identifier`. Returns `None` (with a warning) if there is nothing
/// usable in it.
pub fn map_identifier(
    id: &Identifier,
    terminology: &dyn Terminology,
    report: &mut Report,
) -> Option<Value> {
    let root = id.root.as_deref();
    let extension = id.extension.as_deref();

    let mapped = match (root, extension) {
        (None, Some(value)) => Some(json!({ "value": value })),
        (Some(root), Some(extension)) => {
            map_root_and_extension(root, extension, terminology, report)
        }
        (root, None) => map_without_extension(root, id.null_flavor.as_deref(), terminology, report),
    };
    // `assigningAuthorityName` is the human readable name of the namespace.
    match (mapped, id.assigning_authority_name.as_deref()) {
        (Some(mut v), Some(name)) if !name.trim().is_empty() => {
            v["assigner"] = json!({ "display": name });
            Some(v)
        }
        (mapped, _) => mapped,
    }
}

fn map_root_and_extension(
    root: &str,
    extension: &str,
    terminology: &dyn Terminology,
    report: &mut Report,
) -> Option<Value> {
    // Root is the URI OID: the extension is itself a URL or URN to split.
    if root == URI_OID {
        let split = if extension.starts_with("http://") || extension.starts_with("https://") {
            extension.rsplit_once('/')
        } else if extension.starts_with("urn:") {
            extension.rsplit_once(':')
        } else {
            None
        };
        if let Some((system, value)) = split.filter(|(s, v)| !s.is_empty() && !v.is_empty()) {
            return Some(json!({ "system": system, "value": value }));
        }
    }

    let system = if is_uuid(root) {
        format!("urn:uuid:{}", root.to_ascii_lowercase())
    } else if is_oid(root) {
        terminology.system_for_oid(root)
    } else {
        report.warn(
            "identifier.root.invalid",
            "an id root is neither an OID nor a UUID; the identifier system was omitted",
        );
        return Some(json!({ "value": extension }));
    };
    // Under the NPI system an identifier has to be a real NPI (US Core: 10 digits and a valid
    // check digit). A made-up one (the HL7 examples use some) is kept, under its OID.
    if system == NPI_SYSTEM && !is_valid_npi(extension) {
        report.warn(
            "identifier.npi.invalid",
            "an NPI is not 10 digits with a valid check digit; it was kept under the OID system instead of the NPI system",
        );
        return Some(json!({ "system": format!("urn:oid:{root}"), "value": extension }));
    }
    Some(json!({ "system": system, "value": extension }))
}

const NPI_SYSTEM: &str = "http://hl7.org/fhir/sid/us-npi";

/// 10 digits whose Luhn check, with the card prefix 80840, passes.
fn is_valid_npi(value: &str) -> bool {
    if value.len() != 10 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let digits = format!("80840{value}");
    let sum: u32 = digits
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, b)| {
            let d = u32::from(b - b'0');
            if i % 2 == 1 {
                let doubled = d * 2;
                doubled / 10 + doubled % 10
            } else {
                d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

fn map_without_extension(
    root: Option<&str>,
    null_flavor: Option<&str>,
    terminology: &dyn Terminology,
    report: &mut Report,
) -> Option<Value> {
    let absent = |nf: &str, report: &mut Report| json!({ "extension": [data_absent_reason_extension(nf, report)] });

    let Some(root) = root else {
        // Only a nullFlavor (or nothing at all).
        return match null_flavor {
            Some(nf) => Some(absent(nf, report)),
            None => {
                report.warn(
                    "identifier.empty",
                    "an id had no root, extension or nullFlavor",
                );
                None
            }
        };
    };

    let uuid = is_uuid(root);
    if !uuid && !is_oid(root) {
        report.warn(
            "identifier.root.invalid",
            "an id root is neither an OID nor a UUID; the identifier was omitted",
        );
        return None;
    }

    // A known identifier system (SSN, NPI) with no value, or an explicit nullFlavor: the system is
    // kept and the missing value is explained. Sending the OID as the value would be wrong.
    if null_flavor.is_some() || (!uuid && terminology.is_identifier_system(root)) {
        let system = if uuid {
            format!("urn:uuid:{}", root.to_ascii_lowercase())
        } else {
            terminology.system_for_oid(root)
        };
        let mut identifier = Map::new();
        identifier.insert("system".into(), json!(system));
        identifier.insert("_value".into(), absent(null_flavor.unwrap_or("NI"), report));
        if null_flavor.is_none() {
            report.warn(
                "identifier.value.missing",
                "an id for a known identifier system had no extension; the value was marked absent",
            );
        }
        return Some(Value::Object(identifier));
    }

    // Root only: the root is unique on its own and becomes the value.
    let value = if uuid {
        format!("urn:uuid:{}", root.to_ascii_lowercase())
    } else {
        format!("urn:oid:{root}")
    };
    Some(json!({ "system": URI_SYSTEM, "value": value }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccda_terminology::BuiltinTerminology;

    fn id(root: Option<&str>, extension: Option<&str>, null_flavor: Option<&str>) -> Identifier {
        Identifier {
            root: root.map(Into::into),
            extension: extension.map(Into::into),
            null_flavor: null_flavor.map(Into::into),
            ..Identifier::default()
        }
    }

    fn map(i: &Identifier) -> (Option<Value>, Report) {
        let mut report = Report::default();
        (map_identifier(i, &BuiltinTerminology, &mut report), report)
    }

    // The next tests mirror the rows of the IG table one for one.

    #[test]
    fn known_url_translates_the_oid() {
        let (v, r) = map(&id(
            Some("2.16.840.1.113883.4.1"),
            Some("000-00-0000"),
            None,
        ));
        assert_eq!(
            v.unwrap(),
            json!({ "system": "http://hl7.org/fhir/sid/us-ssn", "value": "000-00-0000" })
        );
        assert!(r.issues.is_empty());
    }

    #[test]
    fn unknown_oid_with_extension_uses_urn_oid() {
        let (v, _) = map(&id(
            Some("2.16.840.1.113883.4.500"),
            Some("12345V7890"),
            None,
        ));
        assert_eq!(
            v.unwrap(),
            json!({ "system": "urn:oid:2.16.840.1.113883.4.500", "value": "12345V7890" })
        );
    }

    #[test]
    fn oid_without_extension_becomes_a_urn_value() {
        let (v, _) = map(&id(Some("2.16.840.1.123.4.50.123456789"), None, None));
        assert_eq!(
            v.unwrap(),
            json!({ "system": "urn:ietf:rfc:3986", "value": "urn:oid:2.16.840.1.123.4.50.123456789" })
        );
    }

    #[test]
    fn uuid_without_extension_is_lowercased() {
        let (v, _) = map(&id(
            Some("67265ED2-35BB-43F8-B9DE-91C5935625E0"),
            None,
            None,
        ));
        assert_eq!(
            v.unwrap(),
            json!({ "system": "urn:ietf:rfc:3986", "value": "urn:uuid:67265ed2-35bb-43f8-b9de-91c5935625e0" })
        );
    }

    #[test]
    fn uuid_with_extension_uses_the_uuid_as_system() {
        let (v, _) = map(&id(
            Some("67265ED2-35BB-43F8-B9DE-91C5935625E0"),
            Some("abcd"),
            None,
        ));
        assert_eq!(
            v.unwrap(),
            json!({ "system": "urn:uuid:67265ed2-35bb-43f8-b9de-91c5935625e0", "value": "abcd" })
        );
    }

    #[test]
    fn extension_only_sets_just_the_value() {
        let (v, _) = map(&id(None, Some("Z1124"), None));
        assert_eq!(v.unwrap(), json!({ "value": "Z1124" }));
    }

    #[test]
    fn uri_root_splits_a_url_extension() {
        let (v, _) = map(&id(
            Some(URI_OID),
            Some("http://myorg.com/patient/1234"),
            None,
        ));
        assert_eq!(
            v.unwrap(),
            json!({ "system": "http://myorg.com/patient", "value": "1234" })
        );
    }

    #[test]
    fn uri_root_splits_a_urn_extension() {
        let (v, _) = map(&id(Some(URI_OID), Some("urn:myNID:myOrg:Z3321"), None));
        assert_eq!(
            v.unwrap(),
            json!({ "system": "urn:myNID:myOrg", "value": "Z3321" })
        );
    }

    #[test]
    fn an_invalid_npi_is_kept_under_its_oid() {
        let (v, r) = map(&id(Some("2.16.840.1.113883.4.6"), Some("1234567890"), None));
        let v = v.unwrap();
        assert_eq!(v["system"], "urn:oid:2.16.840.1.113883.4.6");
        assert_eq!(v["value"], "1234567890");
        assert!(r.issues.iter().any(|i| i.code == "identifier.npi.invalid"));
        let (v, _) = map(&id(Some("2.16.840.1.113883.4.6"), Some("1234567893"), None));
        assert_eq!(v.unwrap()["system"], NPI_SYSTEM);
        assert!(!is_valid_npi("123456789"));
        assert!(!is_valid_npi("12345678a3"));
    }

    #[test]
    fn known_identifier_system_without_extension_marks_the_value_absent() {
        let (v, r) = map(&id(Some("2.16.840.1.113883.4.6"), None, None));
        let v = v.unwrap();
        assert_eq!(v["system"], "http://hl7.org/fhir/sid/us-npi");
        assert!(v.get("value").is_none());
        assert_eq!(v["_value"]["extension"][0]["valueCode"], "unknown");
        assert_eq!(r.issues[0].code, "identifier.value.missing");
    }

    #[test]
    fn null_flavor_with_root_puts_the_reason_on_the_value() {
        let (v, r) = map(&id(Some("2.16.840.1.113883.4.6"), None, Some("UNK")));
        let v = v.unwrap();
        assert_eq!(v["system"], "http://hl7.org/fhir/sid/us-npi");
        assert_eq!(v["_value"]["extension"][0]["valueCode"], "unknown");
        assert!(r.issues.is_empty());
    }

    #[test]
    fn null_flavor_alone_puts_the_reason_on_the_identifier() {
        let (v, _) = map(&id(None, None, Some("NI")));
        assert_eq!(v.unwrap()["extension"][0]["valueCode"], "unknown");
    }

    #[test]
    fn empty_id_is_dropped_with_a_warning() {
        let (v, r) = map(&id(None, None, None));
        assert!(v.is_none());
        assert_eq!(r.issues[0].code, "identifier.empty");
    }

    #[test]
    fn garbage_root_never_leaks_into_messages() {
        let (v, r) = map(&id(Some("not-an-oid-Secret"), Some("abc"), None));
        assert_eq!(v.unwrap(), json!({ "value": "abc" }));
        assert_eq!(r.issues[0].code, "identifier.root.invalid");
        assert!(!r.issues[0].message.contains("Secret"));
    }
}
