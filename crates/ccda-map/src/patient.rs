//! Header `recordTarget/patientRole` to a FHIR `Patient`.

use ccda_model::{
    Address, ClinicalDocument, Code, PersonName, StatementKind, Telecom, Value as Cv,
};
use ccda_terminology::Terminology;
use serde_json::{json, Map, Value};

use crate::{hl7_date_to_fhir_date, map_codeable_concept, map_identifier, Report};

const RACE_URL: &str = "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race";
const ETHNICITY_URL: &str = "http://hl7.org/fhir/us/core/StructureDefinition/us-core-ethnicity";
const BIRTH_SEX_URL: &str = "http://hl7.org/fhir/us/core/StructureDefinition/us-core-birthsex";
const GENDER_IDENTITY_URL: &str =
    "http://hl7.org/fhir/us/core/StructureDefinition/us-core-genderIdentity";
const CDC_RACE_SYSTEM: &str = "urn:oid:2.16.840.1.113883.6.238";
const CDC_RACE_OID: &str = "2.16.840.1.113883.6.238";
const BIRTH_SEX_TEMPLATE: &str = "2.16.840.1.113883.10.20.22.4.200";
const GENDER_IDENTITY_TEMPLATE: &str = "2.16.840.1.113883.10.20.34.3.45";

/// The OMB categories and their names (the extension's `text` needs one when the document gives none).
const RACE_OMB: [(&str, &str); 7] = [
    ("1002-5", "American Indian or Alaska Native"),
    ("2028-9", "Asian"),
    ("2054-5", "Black or African American"),
    ("2076-8", "Native Hawaiian or Other Pacific Islander"),
    ("2106-3", "White"),
    ("UNK", "Unknown"),
    ("ASKU", "Asked but no answer"),
];
const ETHNICITY_OMB: [(&str, &str); 4] = [
    ("2135-2", "Hispanic or Latino"),
    ("2186-5", "Not Hispanic or Latino"),
    ("UNK", "Unknown"),
    ("ASKU", "Asked but no answer"),
];

/// The `id` of the one Patient a document describes. Other resources refer to it.
pub const PATIENT_ID: &str = "patient-1";

/// Map the document's patient to a FHIR `Patient` resource.
///
/// Scope today: identifiers, name, gender and birth date. Address, telecom, race, ethnicity and
/// the rest of USCDI Patient Demographics come with the coverage matrix.
pub fn map_patient(
    doc: &ClinicalDocument,
    terminology: &dyn Terminology,
    report: &mut Report,
) -> Value {
    let patient = &doc.patient;
    let mut resource = Map::new();
    resource.insert("resourceType".into(), json!("Patient"));
    resource.insert("id".into(), json!(PATIENT_ID));

    let identifiers: Vec<Value> = patient
        .identifiers
        .iter()
        .filter_map(|id| map_identifier(id, terminology, report))
        .collect();
    if !identifiers.is_empty() {
        resource.insert("identifier".into(), json!(identifiers));
    }

    if let Some(code) = &patient.gender_code {
        resource.insert("gender".into(), json!(map_gender(code, report)));
    }

    let names: Vec<PersonName> =
        if patient.names.is_empty() && (patient.family.is_some() || !patient.given.is_empty()) {
            vec![PersonName {
                given: patient.given.clone(),
                family: patient.family.clone(),
                ..PersonName::default()
            }]
        } else {
            patient.names.clone()
        };
    let names: Vec<Value> = names.iter().filter_map(human_name).collect();
    if !names.is_empty() {
        resource.insert("name".into(), json!(names));
    }
    let addresses: Vec<Value> = patient.addresses.iter().filter_map(address).collect();
    if !addresses.is_empty() {
        resource.insert("address".into(), json!(addresses));
    }
    let telecoms: Vec<Value> = patient.telecoms.iter().filter_map(telecom).collect();
    if !telecoms.is_empty() {
        resource.insert("telecom".into(), json!(telecoms));
    }
    if let Some(code) = &patient.marital_status {
        if let Some(c) = map_codeable_concept(code, terminology, report) {
            resource.insert("maritalStatus".into(), c);
        }
    }

    let mut extensions: Vec<Value> = Vec::new();
    extensions.extend(category_extension(
        RACE_URL,
        &patient.race_codes,
        &RACE_OMB,
        5,
        report,
    ));
    extensions.extend(category_extension(
        ETHNICITY_URL,
        &patient.ethnic_codes,
        &ETHNICITY_OMB,
        1,
        report,
    ));
    extensions.extend(sex_and_gender_identity(doc, terminology, report));
    if !extensions.is_empty() {
        resource.insert("extension".into(), json!(extensions));
    }

    let communication: Vec<Value> = patient
        .languages
        .iter()
        .filter_map(|l| {
            let code = l.code.as_deref()?;
            let mut c = Map::new();
            c.insert(
                "language".into(),
                json!({ "coding": [{ "system": "urn:ietf:bcp:47", "code": code }] }),
            );
            if let Some(p) = l.preferred {
                c.insert("preferred".into(), json!(p));
            }
            Some(Value::Object(c))
        })
        .collect();
    if !communication.is_empty() {
        resource.insert("communication".into(), json!(communication));
    }

    let contacts: Vec<Value> = patient
        .guardians
        .iter()
        .filter_map(|g| {
            let mut c = Map::new();
            if let Some(rel) = g
                .relationship
                .as_ref()
                .and_then(|r| map_codeable_concept(r, terminology, report))
            {
                c.insert("relationship".into(), json!([rel]));
            }
            if let Some(n) = g.names.iter().find_map(human_name) {
                c.insert("name".into(), n);
            }
            let t: Vec<Value> = g.telecoms.iter().filter_map(telecom).collect();
            if !t.is_empty() {
                c.insert("telecom".into(), json!(t));
            }
            if let Some(a) = g.addresses.iter().find_map(address) {
                c.insert("address".into(), a);
            }
            let has_detail =
                c.contains_key("name") || c.contains_key("telecom") || c.contains_key("address");
            has_detail.then_some(Value::Object(c))
        })
        .collect();
    if !contacts.is_empty() {
        resource.insert("contact".into(), json!(contacts));
    }

    if let Some(raw) = &patient.birth_time {
        match hl7_date_to_fhir_date(raw) {
            Some(date) => {
                resource.insert("birthDate".into(), json!(date));
            }
            None => report.warn(
                "patient.birthTime.invalid",
                "birthTime is not a valid HL7 timestamp; birthDate was omitted",
            ),
        }
    }

    Value::Object(resource)
}

fn human_name(n: &PersonName) -> Option<Value> {
    if n.family.is_none() && n.given.is_empty() {
        return None;
    }
    let mut out = Map::new();
    match n.use_code.as_deref() {
        Some("L") => {
            out.insert("use".into(), json!("official"));
        }
        Some("P") => {
            out.insert("use".into(), json!("anonymous"));
        }
        _ => {}
    }
    if let Some(f) = &n.family {
        out.insert("family".into(), json!(f));
    }
    for (key, parts) in [
        ("given", &n.given),
        ("prefix", &n.prefix),
        ("suffix", &n.suffix),
    ] {
        if !parts.is_empty() {
            out.insert(key.into(), json!(parts));
        }
    }
    Some(Value::Object(out))
}

pub(crate) fn address(a: &Address) -> Option<Value> {
    let mut out = Map::new();
    match a.use_code.as_deref() {
        Some("H" | "HP") => {
            out.insert("use".into(), json!("home"));
        }
        Some("WP" | "DIR") => {
            out.insert("use".into(), json!("work"));
        }
        Some("TMP" | "HV") => {
            out.insert("use".into(), json!("temp"));
        }
        Some("BAD") => {
            out.insert("use".into(), json!("old"));
        }
        _ => {}
    }
    if !a.lines.is_empty() {
        out.insert("line".into(), json!(a.lines));
    }
    for (key, v) in [
        ("city", &a.city),
        ("state", &a.state),
        ("postalCode", &a.postal_code),
        ("country", &a.country),
    ] {
        if let Some(v) = v {
            out.insert(key.into(), json!(v));
        }
    }
    (out.keys().any(|k| k != "use")).then_some(Value::Object(out))
}

pub(crate) fn telecom(t: &Telecom) -> Option<Value> {
    let raw = t.value.as_deref()?;
    let (system, value) = if let Some(v) = raw.strip_prefix("tel:") {
        ("phone", v)
    } else if let Some(v) = raw.strip_prefix("fax:") {
        ("fax", v)
    } else if let Some(v) = raw.strip_prefix("mailto:") {
        ("email", v)
    } else if raw.starts_with("http://") || raw.starts_with("https://") {
        ("url", raw)
    } else {
        ("other", raw)
    };
    let mut out = Map::new();
    out.insert("system".into(), json!(system));
    out.insert("value".into(), json!(value));
    match t.use_code.as_deref() {
        Some("H" | "HP") => {
            out.insert("use".into(), json!("home"));
        }
        Some("WP" | "DIR") => {
            out.insert("use".into(), json!("work"));
        }
        Some("MC") => {
            out.insert("use".into(), json!("mobile"));
        }
        Some("TMP" | "HV") => {
            out.insert("use".into(), json!("temp"));
        }
        Some("BAD") => {
            out.insert("use".into(), json!("old"));
        }
        _ => {}
    }
    Some(Value::Object(out))
}

/// The US Core race or ethnicity extension: OMB categories, detailed codes and the required text.
fn category_extension(
    url: &str,
    codes: &[Code],
    omb: &[(&str, &str)],
    max_omb: usize,
    report: &mut Report,
) -> Option<Value> {
    let mut categories: Vec<(String, String)> = Vec::new();
    let mut detailed: Vec<(String, Option<String>)> = Vec::new();
    for c in codes {
        let value = match (&c.code, &c.null_flavor) {
            (Some(code), _) => code.clone(),
            (None, Some(nf)) if nf == "UNK" || nf == "ASKU" => nf.clone(),
            _ => continue,
        };
        if c.code.is_some() && c.code_system.as_deref().is_some_and(|s| s != CDC_RACE_OID) {
            report.warn(
                "patient.race.system",
                "a race or ethnicity code is not from the CDC Race and Ethnicity code system and was omitted",
            );
            continue;
        }
        match omb.iter().find(|(k, _)| *k == value) {
            Some((k, name)) => {
                let display = c.display_name.clone().unwrap_or_else(|| (*name).to_owned());
                if !categories.iter().any(|(code, _)| code == k) {
                    categories.push(((*k).to_owned(), display));
                }
            }
            None => {
                if !detailed.iter().any(|(code, _)| *code == value) {
                    detailed.push((value, c.display_name.clone()));
                }
            }
        }
    }
    if categories.len() > max_omb {
        report.warn(
            "patient.race.too-many",
            "more race or ethnicity categories than US Core allows; the extra ones were omitted",
        );
        categories.truncate(max_omb);
    }
    let text = categories
        .first()
        .map(|(_, d)| d.clone())
        .or_else(|| detailed.iter().find_map(|(_, d)| d.clone()))
        .or_else(|| detailed.first().map(|(c, _)| c.clone()))?;
    let coding = |code: &str, display: Option<&str>| {
        let mut c = json!({ "system": CDC_RACE_SYSTEM, "code": code });
        if let Some(d) = display {
            c["display"] = json!(d);
        }
        c
    };
    let mut ext: Vec<Value> = Vec::new();
    for (code, display) in &categories {
        ext.push(json!({ "url": "ombCategory", "valueCoding": coding(code, Some(display)) }));
    }
    for (code, display) in &detailed {
        ext.push(json!({ "url": "detailed", "valueCoding": coding(code, display.as_deref()) }));
    }
    ext.push(json!({ "url": "text", "valueString": text }));
    Some(json!({ "url": url, "extension": ext }))
}

/// Birth sex and gender identity observations belong on the Patient as extensions.
fn sex_and_gender_identity(
    doc: &ClinicalDocument,
    terminology: &dyn Terminology,
    report: &mut Report,
) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind != StatementKind::Observation {
                continue;
            }
            let Some(Cv::Coded(value)) = s.values.first() else {
                continue;
            };
            if s.has_template(BIRTH_SEX_TEMPLATE) {
                match value.code.as_deref() {
                    Some(c @ ("M" | "F")) => {
                        out.push(json!({ "url": BIRTH_SEX_URL, "valueCode": c }));
                    }
                    Some("UN" | "UNK") => {
                        out.push(json!({ "url": BIRTH_SEX_URL, "valueCode": "UNK" }));
                    }
                    _ => report.warn(
                        "patient.birth-sex.unrecognized",
                        "a birth sex value is not M, F or UN and was omitted",
                    ),
                }
            } else if s.has_template(GENDER_IDENTITY_TEMPLATE) {
                if let Some(c) = map_codeable_concept(value, terminology, report) {
                    out.push(json!({ "url": GENDER_IDENTITY_URL, "valueCodeableConcept": c }));
                }
            }
        }
    }
    out
}

/// HL7 v3 AdministrativeGender to FHIR `Patient.gender`.
///
/// M and F are equivalent; UN maps to `other` ("wider"). Source: the IG's ConceptMap
/// `ConceptMap-CF-AdministrativeGender`. Codes outside that map are our own fallback.
fn map_gender(code: &str, report: &mut Report) -> &'static str {
    match code {
        "M" => "male",
        "F" => "female",
        "UN" => "other",
        _ => {
            report.warn(
                "patient.gender.unrecognized",
                "administrativeGenderCode is not M, F or UN; gender was set to unknown",
            );
            "unknown"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccda_model::{Identifier, Patient};
    use ccda_terminology::BuiltinTerminology;

    fn doc_with(patient: Patient) -> ClinicalDocument {
        ClinicalDocument {
            id: None,
            code: None,
            effective_time: None,
            patient,
            title: None,
            authors: Vec::new(),
            custodian: None,
            confidentiality: None,
            language: None,
            service_events: Vec::new(),
            encompassing_encounter: None,
            sections: Vec::new(),
        }
    }

    fn map(patient: Patient) -> (Value, Report) {
        let mut report = Report::default();
        let resource = map_patient(&doc_with(patient), &BuiltinTerminology, &mut report);
        (resource, report)
    }

    #[test]
    fn maps_identifiers_name_gender_and_birth_date() {
        let (resource, report) = map(Patient {
            given: vec!["Alice".into(), "Q".into()],
            family: Some("Example".into()),
            birth_time: Some("19700101".into()),
            gender_code: Some("F".into()),
            identifiers: vec![Identifier {
                root: Some("2.16.840.1.113883.19.5".into()),
                extension: Some("MRN-1".into()),
                null_flavor: None,
            }],
            ..Patient::default()
        });
        assert_eq!(resource["resourceType"], "Patient");
        assert_eq!(
            resource["identifier"][0],
            json!({ "system": "urn:oid:2.16.840.1.113883.19.5", "value": "MRN-1" })
        );
        assert_eq!(resource["name"][0]["family"], "Example");
        assert_eq!(resource["name"][0]["given"], json!(["Alice", "Q"]));
        assert_eq!(resource["gender"], "female");
        assert_eq!(resource["birthDate"], "1970-01-01");
        assert!(report.issues.is_empty());
    }

    #[test]
    fn gender_follows_the_ig_concept_map() {
        for (code, expected) in [("M", "male"), ("F", "female"), ("UN", "other")] {
            let (resource, report) = map(Patient {
                gender_code: Some(code.into()),
                ..Patient::default()
            });
            assert_eq!(resource["gender"], expected);
            assert!(report.issues.is_empty());
        }
    }

    #[test]
    fn unrecognized_gender_becomes_unknown_with_a_warning() {
        let (resource, report) = map(Patient {
            gender_code: Some("X".into()),
            ..Patient::default()
        });
        assert_eq!(resource["gender"], "unknown");
        assert!(report.has_warnings());
        assert_eq!(report.issues[0].code, "patient.gender.unrecognized");
        assert!(!report.issues[0].message.contains('X'));
    }

    #[test]
    fn invalid_birth_time_is_omitted_with_a_warning() {
        let (resource, report) = map(Patient {
            birth_time: Some("19701301".into()),
            ..Patient::default()
        });
        assert!(resource.get("birthDate").is_none());
        assert_eq!(report.issues[0].code, "patient.birthTime.invalid");
    }

    #[test]
    fn unusable_identifiers_are_dropped_with_a_warning() {
        let (resource, report) = map(Patient {
            identifiers: vec![Identifier::default()],
            ..Patient::default()
        });
        assert!(resource.get("identifier").is_none());
        assert_eq!(report.issues[0].code, "identifier.empty");
    }

    #[test]
    fn empty_patient_yields_a_bare_resource() {
        let (resource, report) = map(Patient::default());
        assert_eq!(
            resource,
            json!({ "resourceType": "Patient", "id": PATIENT_ID })
        );
        assert!(report.issues.is_empty());
    }

    fn parse_doc(patient_xml: &str, body: &str) -> (Value, Report) {
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:sdtc="urn:hl7-org:sdtc" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<recordTarget><patientRole>{patient_xml}</patientRole></recordTarget>
<component><structuredBody><component><section>{body}</section></component></structuredBody></component>
</ClinicalDocument>"#
        );
        let doc = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let resource = map_patient(&doc, &BuiltinTerminology, &mut report);
        (resource, report)
    }

    #[test]
    fn names_addresses_and_telecoms_keep_their_parts_and_uses() {
        let (r, _) = parse_doc(
            r#"<id root="1.2" extension="A"/>
<addr use="HP"><streetAddressLine>1 Main St</streetAddressLine><streetAddressLine>Apt 2</streetAddressLine><city>Town</city><state>CA</state><postalCode>90000</postalCode><country>US</country></addr>
<addr nullFlavor="NI"/>
<telecom use="MC" value="tel:+1-555-555-0100"/>
<telecom use="WP" value="mailto:a@example.com"/>
<telecom nullFlavor="UNK"/>
<patient>
  <name use="L"><prefix>Dr</prefix><given>Ann</given><given>B</given><family>Example</family><suffix>Jr</suffix></name>
  <name><given>Annie</given></name>
</patient>"#,
            "",
        );
        assert_eq!(r["name"][0]["use"], "official");
        assert_eq!(r["name"][0]["prefix"][0], "Dr");
        assert_eq!(r["name"][0]["suffix"][0], "Jr");
        assert_eq!(r["name"].as_array().unwrap().len(), 2);
        assert_eq!(r["address"].as_array().unwrap().len(), 1);
        assert_eq!(r["address"][0]["line"][1], "Apt 2");
        assert_eq!(r["address"][0]["use"], "home");
        assert_eq!(r["telecom"][0]["system"], "phone");
        assert_eq!(r["telecom"][0]["use"], "mobile");
        assert_eq!(r["telecom"][1]["system"], "email");
        assert_eq!(r["telecom"][1]["value"], "a@example.com");
        assert_eq!(r["telecom"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn race_and_ethnicity_become_us_core_extensions() {
        let (r, _) = parse_doc(
            r#"<patient>
  <raceCode code="2106-3" displayName="White" codeSystem="2.16.840.1.113883.6.238"/>
  <sdtc:raceCode code="2108-9" displayName="European" codeSystem="2.16.840.1.113883.6.238"/>
  <ethnicGroupCode code="2186-5" codeSystem="2.16.840.1.113883.6.238"/>
</patient>"#,
            "",
        );
        let race = &r["extension"][0];
        assert_eq!(
            race["url"],
            "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race"
        );
        assert_eq!(race["extension"][0]["url"], "ombCategory");
        assert_eq!(race["extension"][0]["valueCoding"]["code"], "2106-3");
        assert_eq!(race["extension"][1]["url"], "detailed");
        assert_eq!(
            race["extension"][2],
            json!({ "url": "text", "valueString": "White" })
        );
        let eth = &r["extension"][1];
        assert_eq!(
            eth["extension"][0]["valueCoding"]["display"],
            "Not Hispanic or Latino"
        );
    }

    #[test]
    fn race_with_a_null_flavor_is_unknown_and_foreign_systems_are_dropped() {
        let (r, report) = parse_doc(r#"<patient><raceCode nullFlavor="UNK"/></patient>"#, "");
        assert_eq!(
            r["extension"][0]["extension"][0]["valueCoding"]["code"],
            "UNK"
        );
        let (r, report2) = parse_doc(
            r#"<patient><raceCode code="X" codeSystem="1.2.3"/></patient>"#,
            "",
        );
        assert!(r.get("extension").is_none());
        assert!(report.issues.is_empty());
        assert_eq!(report2.issues[0].code, "patient.race.system");
    }

    #[test]
    fn birth_sex_and_gender_identity_come_from_social_history() {
        let body = r#"<entry><observation classCode="OBS" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.200"/>
  <code code="76689-9" codeSystem="2.16.840.1.113883.6.1"/>
  <value xsi:type="CD" code="F" codeSystem="2.16.840.1.113883.5.1"/>
</observation></entry>
<entry><observation classCode="OBS" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.34.3.45"/>
  <code code="76691-5" codeSystem="2.16.840.1.113883.6.1"/>
  <value xsi:type="CD" code="446141000124107" codeSystem="2.16.840.1.113883.6.96"/>
</observation></entry>"#;
        let (r, _) = parse_doc("<patient/>", body);
        let ext = r["extension"].as_array().unwrap();
        assert!(ext.iter().any(|e| e["valueCode"] == "F"));
        assert!(ext
            .iter()
            .any(|e| e["valueCodeableConcept"]["coding"][0]["code"] == "446141000124107"));
    }

    #[test]
    fn language_marital_status_and_guardian() {
        let (r, _) = parse_doc(
            r#"<patient>
  <maritalStatusCode code="M" codeSystem="2.16.840.1.113883.5.2"/>
  <guardian>
    <code code="GRD" codeSystem="2.16.840.1.113883.5.111"/>
    <telecom value="tel:555-0101"/>
    <guardianPerson><name><given>Pat</given><family>Parent</family></name></guardianPerson>
  </guardian>
  <languageCommunication><languageCode code="es"/><preferenceInd value="true"/></languageCommunication>
</patient>"#,
            "",
        );
        assert_eq!(
            r["maritalStatus"]["coding"][0]["system"],
            "http://terminology.hl7.org/CodeSystem/v3-MaritalStatus"
        );
        assert_eq!(r["communication"][0]["language"]["coding"][0]["code"], "es");
        assert_eq!(r["communication"][0]["preferred"], true);
        assert_eq!(r["contact"][0]["name"]["family"], "Parent");
        assert_eq!(
            r["contact"][0]["relationship"][0]["coding"][0]["code"],
            "GRD"
        );
    }
}
