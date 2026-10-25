//! Allergy Concern Acts to FHIR `AllergyIntolerance`.
//!
//! Rules come from the IG page "CF-allergies", its CSV, and the ConceptMaps
//! `CF-AllergyIntoleranceType`, `-Category`, `-AllergyStatus`, `-NoKnownAllergies`, `-Severity`
//! and `-Criticality` (HL7/ccda-on-fhir). Each Allergy Intolerance Observation inside a concern
//! becomes its own resource.

use ccda_model::{ClinicalDocument, Statement, Value as Cv};
use serde_json::{json, Map, Value};

use crate::author::{apply_authors, RECORDER_FIELDS};
use crate::util::{concept, is_coded, nested, start_of, LOINC, SNOMED};
use crate::{
    data_absent_reason_extension, map_codeable_concept, map_datetime, map_identifier, Context,
};

const CONCERN_ACT: &str = "2.16.840.1.113883.10.20.22.4.30";
const ALLERGY_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.7";
const SEVERITY_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.8";
const REACTION_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.9";

const CLINICAL_STATUS_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/allergyintolerance-clinical";
const ABATEMENT_URL: &str = "http://hl7.org/fhir/StructureDefinition/allergyintolerance-abatement";
const EXPOSURE_RISK_URL: &str =
    "http://hl7.org/fhir/StructureDefinition/allergyintolerance-substanceExposureRisk";
const EXPOSURE_RISK_SYSTEM: &str = "http://hl7.org/fhir/allerg-intol-substance-exp-risk";
const SNOMED_URI: &str = "http://snomed.info/sct";

/// Map every Allergy Concern Act in the document to `AllergyIntolerance` resources.
pub fn map_allergies(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let concern = &entry.statement;
            if !concern.has_template(CONCERN_ACT) {
                continue;
            }
            let observations: Vec<_> = concern
                .related_by("SUBJ")
                .filter(|s| s.has_template(ALLERGY_OBSERVATION))
                .collect();
            if observations.is_empty() {
                ctx.report.warn(
                    "allergy.concern.no-observation",
                    "an Allergy Concern Act has no Allergy Intolerance Observation; nothing was created",
                );
            }
            for observation in observations {
                out.push(map_observation(concern, observation, ctx));
            }
        }
    }
    out
}

/// `CF-AllergyIntoleranceType`: SNOMED allergy-type value to `type`.
fn type_of(code: &str) -> Option<&'static str> {
    Some(match code {
        "414285001" | "416098002" | "419199007" => "allergy",
        "235719002" | "59037007" => "intolerance",
        _ => return None,
    })
}

/// `CF-AllergyIntoleranceCategory`: SNOMED allergy-type value to `category`.
fn category_of(code: &str) -> Option<&'static str> {
    Some(match code {
        "235719002" | "414285001" | "418471000" => "food",
        "416098002" | "419511003" | "59037007" => "medication",
        _ => return None,
    })
}

/// `CF-NoKnownAllergies`: the value's "allergy to X" concept to the "no known X allergy" concept.
fn no_known_allergy(code: &str) -> Option<&'static str> {
    Some(match code {
        "414285001" => "429625007",
        "416098002" => "409137002",
        "419199007" => "716186003",
        _ => return None,
    })
}

/// `CF-AllergyStatus`.
fn status_of(code: &str) -> Option<&'static str> {
    Some(match code {
        "413322009" => "resolved",
        "73425007" => "inactive",
        "55561003" => "active",
        _ => return None,
    })
}

/// `CF-Severity`.
fn severity_of(code: &str) -> Option<&'static str> {
    Some(match code {
        "255604002" => "mild",
        "6736007" => "moderate",
        "24484000" => "severe",
        _ => return None,
    })
}

/// `CF-Criticality`.
fn criticality_of(code: &str) -> Option<&'static str> {
    Some(match code {
        "CRITH" => "high",
        "CRITL" => "low",
        "CRITU" => "unable-to-assess",
        _ => return None,
    })
}

fn coded_value(statement: &Statement) -> Option<&ccda_model::Code> {
    match statement.values.first() {
        Some(Cv::Coded(code)) => Some(code),
        _ => None,
    }
}

fn snomed_code(code: &ccda_model::Code) -> Option<&str> {
    code.code_system
        .as_deref()
        .filter(|oid| *oid == SNOMED)
        .and(code.code.as_deref())
}

fn clinical_status(
    observation: &Statement,
    concern: &Statement,
    ctx: &mut Context,
) -> &'static str {
    if let Some(value) = nested(observation)
        .find(|s| is_coded(s, "33999-4", LOINC))
        .and_then(coded_value)
    {
        match snomed_code(value).and_then(status_of) {
            Some(status) => return status,
            None if value.null_flavor.is_none() => ctx.report.warn(
                "allergy.status.unmapped",
                "an Allergy Status value is not in the IG's status map; the concern status was used instead",
            ),
            None => {}
        }
    }
    match concern.status_code.as_deref() {
        Some("active") => "active",
        // The IG has no ActStatus map for allergies; this mirrors the Condition map.
        Some("aborted" | "completed" | "suspended") => "inactive",
        Some(_) => {
            ctx.report.warn(
                "allergy.status.unmapped",
                "a concern statusCode is not recognized; clinicalStatus defaulted to active",
            );
            "active"
        }
        None => "active",
    }
}

/// The substance (`participant/participantRole/playingEntity/code`).
fn substance(observation: &Statement) -> Option<&ccda_model::Code> {
    observation
        .participants
        .iter()
        .find_map(|p| p.playing_entity_code.as_ref())
}

fn map_code(
    observation: &Statement,
    value: Option<&ccda_model::Code>,
    resource: &mut Map<String, Value>,
    extensions: &mut Vec<Value>,
    ctx: &mut Context,
) {
    let substance = substance(observation);
    let concept_of = |code: &ccda_model::Code, ctx: &mut Context| {
        map_codeable_concept(code, ctx.terminology, ctx.report)
    };

    if !observation.negation_ind {
        match substance {
            Some(code) => {
                if let Some(c) = concept_of(code, ctx) {
                    resource.insert("code".into(), c);
                } else if let Some(nf) = code.null_flavor.as_deref() {
                    let ext = data_absent_reason_extension(nf, ctx.report);
                    resource.insert("code".into(), json!({ "extension": [ext] }));
                } else {
                    ctx.report.warn(
                        "allergy.code.missing",
                        "an allergen has no usable code; code was omitted",
                    );
                }
            }
            None => ctx.report.warn(
                "allergy.code.missing",
                "an Allergy Intolerance Observation has no allergen; code was omitted",
            ),
        }
        return;
    }

    // Negated: "no known allergy".
    let substance_concept = substance.and_then(|c| concept_of(c, ctx));
    if let Some(substance_concept) = substance_concept {
        // A coded substance: the IG's fallback when no negated concept exists is the
        // substanceExposureRisk extension, which replaces `code`.
        ctx.report.info(
            "allergy.negated.substance-exposure-risk",
            "a negated allergy was mapped with the substanceExposureRisk extension; it has no code, so it is not US Core conformant",
        );
        extensions.push(json!({
            "url": EXPOSURE_RISK_URL,
            "extension": [
                { "url": "substance", "valueCodeableConcept": substance_concept },
                { "url": "exposureRisk", "valueCodeableConcept": concept(EXPOSURE_RISK_SYSTEM, "no-known-reaction-risk") }
            ]
        }));
        return;
    }
    match value.and_then(snomed_code).and_then(no_known_allergy) {
        Some(code) => {
            resource.insert("code".into(), concept(SNOMED_URI, code));
        }
        None => {
            ctx.report.info(
                "allergy.negated.no-known-code",
                "a negated allergy has no matching 'no known allergy' concept; code.text was used",
            );
            resource.insert("code".into(), json!({ "text": "no known allergies" }));
        }
    }
}

fn severity_value(statement: &Statement, ctx: &mut Context) -> Option<&'static str> {
    let value = coded_value(statement)?;
    let mapped = snomed_code(value).and_then(severity_of);
    if mapped.is_none() && value.null_flavor.is_none() {
        ctx.report.warn(
            "allergy.severity.unmapped",
            "a severity value is not in the IG's severity map and was omitted",
        );
    }
    mapped
}

fn is_severity(statement: &Statement) -> bool {
    statement.has_template(SEVERITY_OBSERVATION)
        || statement
            .code
            .as_ref()
            .is_some_and(|c| c.code.as_deref() == Some("SEV"))
}

fn map_reactions(observation: &Statement, ctx: &mut Context) -> Vec<Value> {
    // Severity recorded on the allergy itself applies to every reaction that has none of its own.
    let inherited = nested(observation)
        .find(|s| is_severity(s))
        .and_then(|s| severity_value(s, ctx));

    let mut reactions = Vec::new();
    for reaction in observation
        .related_by("MFST")
        .filter(|s| s.has_template(REACTION_OBSERVATION))
    {
        let mut out = Map::new();
        if let Some(code) = coded_value(reaction) {
            if let Some(c) = map_codeable_concept(code, ctx.terminology, ctx.report) {
                out.insert("manifestation".into(), json!([c]));
            } else if let Some(nf) = code.null_flavor.as_deref() {
                let ext = data_absent_reason_extension(nf, ctx.report);
                out.insert("manifestation".into(), json!([{ "extension": [ext] }]));
            }
        }
        if !out.contains_key("manifestation") {
            ctx.report.warn(
                "allergy.reaction.no-manifestation",
                "a reaction has no usable value; the reaction was omitted",
            );
            continue;
        }
        if let Some(raw) = start_of(reaction).and_then(|p| p.value) {
            if let Some(onset) = map_datetime(&raw, ctx.report) {
                out.insert("onset".into(), json!(onset));
            }
        }
        let own = nested(reaction)
            .find(|s| is_severity(s))
            .and_then(|s| severity_value(s, ctx));
        if let Some(severity) = own.or(inherited) {
            out.insert("severity".into(), json!(severity));
        }
        reactions.push(Value::Object(out));
    }
    if inherited.is_some() && reactions.is_empty() {
        ctx.report.warn(
            "allergy.severity.no-reaction",
            "a severity is recorded on an allergy with no reaction; FHIR keeps severity on reactions, so it was omitted",
        );
    }
    reactions
}

fn map_observation(concern: &Statement, observation: &Statement, ctx: &mut Context) -> Value {
    let mut resource = Map::new();
    let mut extensions: Vec<Value> = Vec::new();
    resource.insert("resourceType".into(), json!("AllergyIntolerance"));
    resource.insert("id".into(), json!(ctx.next_id("allergy")));

    let identifiers: Vec<Value> = observation
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        resource.insert("identifier".into(), json!(identifiers));
    }

    let status = clinical_status(observation, concern, ctx);
    resource.insert(
        "clinicalStatus".into(),
        concept(CLINICAL_STATUS_SYSTEM, status),
    );

    let value = coded_value(observation);
    if let Some(code) = value.and_then(snomed_code) {
        if let Some(t) = type_of(code) {
            resource.insert("type".into(), json!(t));
        }
        if let Some(c) = category_of(code) {
            resource.insert("category".into(), json!([c]));
        }
        if type_of(code).is_none() && category_of(code).is_none() {
            ctx.report.info(
                "allergy.value.unmatched",
                "an allergy type value has no type or category in the IG's maps",
            );
        }
    }

    map_code(observation, value, &mut resource, &mut extensions, ctx);

    if let Some(time) = observation.effective_time.as_ref() {
        if let Some(raw) = start_of(observation).and_then(|p| p.value) {
            if let Some(onset) = map_datetime(&raw, ctx.report) {
                resource.insert("onsetDateTime".into(), json!(onset));
            }
        }
        if let Some(date) = time
            .high
            .as_ref()
            .and_then(|h| h.value.as_deref())
            .and_then(|raw| map_datetime(raw, ctx.report))
        {
            extensions.push(json!({ "url": ABATEMENT_URL, "valueDateTime": date }));
        }
    }

    if let Some(criticality) = nested(observation)
        .find(|s| is_coded(s, "82606-5", LOINC))
        .and_then(coded_value)
        .and_then(|c| c.code.as_deref())
        .and_then(criticality_of)
    {
        resource.insert("criticality".into(), json!(criticality));
    }

    let reactions = map_reactions(observation, ctx);
    if !reactions.is_empty() {
        resource.insert("reaction".into(), json!(reactions));
    }

    if !extensions.is_empty() {
        resource.insert("extension".into(), json!(extensions));
    }
    resource.insert("patient".into(), json!({ "reference": ctx.patient }));
    let authors: Vec<_> = if observation.authors.is_empty() {
        concern.authors.iter().collect()
    } else {
        observation.authors.iter().collect()
    };
    apply_authors(&mut resource, RECORDER_FIELDS, &authors, ctx);
    Value::Object(resource)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn document(concern_status: &str, observations: &str) -> String {
        format!(
            r##"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section>
  <code code="48765-2" codeSystem="2.16.840.1.113883.6.1"/>
  <text><paragraph ID="a1">Penicillin</paragraph><paragraph ID="r1">Hives</paragraph></text>
  <entry><act classCode="ACT" moodCode="EVN">
    <templateId root="2.16.840.1.113883.10.20.22.4.30"/>
    <code code="CONC" codeSystem="2.16.840.1.113883.5.6"/>
    {concern_status}
    {observations}
  </act></entry>
</section></component></structuredBody></component></ClinicalDocument>"##
        )
    }

    fn observation(
        attrs: &str,
        time: &str,
        value_code: &str,
        participant: &str,
        extras: &str,
    ) -> String {
        format!(
            r#"<entryRelationship typeCode="SUBJ"><observation classCode="OBS" moodCode="EVN" {attrs}>
  <templateId root="2.16.840.1.113883.10.20.22.4.7"/>
  <id root="1.2.3" extension="A-1"/>
  <code code="ASSERTION" codeSystem="2.16.840.1.113883.5.4"/>
  {time}
  <value xsi:type="CD" code="{value_code}" codeSystem="2.16.840.1.113883.6.96"/>
  {participant}
  {extras}
</observation></entryRelationship>"#
        )
    }

    fn penicillin() -> &'static str {
        r##"<participant typeCode="CSM"><participantRole classCode="MANU"><playingEntity classCode="MMAT">
  <code code="7980" codeSystem="2.16.840.1.113883.6.88" displayName="Penicillin G"><originalText><reference value="#a1"/></originalText></code>
</playingEntity></participantRole></participant>"##
    }

    fn reaction(value: &str, extras: &str) -> String {
        format!(
            r##"<entryRelationship typeCode="MFST" inversionInd="true"><observation classCode="OBS" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.9"/>
  <effectiveTime><low value="20200301"/></effectiveTime>
  {value}
  {extras}
</observation></entryRelationship>"##
        )
    }

    const HIVES: &str = r##"<value xsi:type="CD" code="247472004" codeSystem="2.16.840.1.113883.6.96" displayName="Hives"><originalText><reference value="#r1"/></originalText></value>"##;

    fn severity(code: &str) -> String {
        format!(
            r#"<entryRelationship typeCode="SUBJ" inversionInd="true"><observation classCode="OBS" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.8"/>
  <code code="SEV" codeSystem="2.16.840.1.113883.5.4"/>
  <value xsi:type="CD" code="{code}" codeSystem="2.16.840.1.113883.6.96"/>
</observation></entryRelationship>"#
        )
    }

    fn run(xml: &str) -> (Vec<Value>, Report) {
        let doc = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_allergies(&doc, &mut ctx);
        (out, report)
    }

    fn codes(report: &Report) -> Vec<&'static str> {
        report.issues.iter().map(|i| i.code).collect()
    }

    const ACTIVE: &str = r#"<statusCode code="active"/>"#;

    #[test]
    fn drug_allergy_with_reaction_severity_and_criticality() {
        let criticality = r#"<entryRelationship typeCode="SUBJ" inversionInd="true"><observation classCode="OBS" moodCode="EVN">
  <code code="82606-5" codeSystem="2.16.840.1.113883.6.1"/>
  <value xsi:type="CD" code="CRITH" codeSystem="2.16.840.1.113883.5.1063"/>
</observation></entryRelationship>"#;
        let extras = format!("{}{}", reaction(HIVES, &severity("6736007")), criticality);
        let xml = document(
            ACTIVE,
            &observation(
                "",
                r#"<effectiveTime><low value="20100101"/></effectiveTime>"#,
                "416098002",
                penicillin(),
                &extras,
            ),
        );
        let (out, report) = run(&xml);
        assert_eq!(
            out[0],
            json!({
                "resourceType": "AllergyIntolerance",
                "id": "allergy-1",
                "identifier": [{ "system": "urn:oid:1.2.3", "value": "A-1" }],
                "clinicalStatus": { "coding": [{ "system": CLINICAL_STATUS_SYSTEM, "code": "active" }] },
                "type": "allergy",
                "category": ["medication"],
                "criticality": "high",
                "code": {
                    "coding": [{ "system": "http://www.nlm.nih.gov/research/umls/rxnorm", "code": "7980", "display": "Penicillin G" }],
                    "text": "Penicillin"
                },
                "onsetDateTime": "2010-01-01",
                "reaction": [{
                    "manifestation": [{
                        "coding": [{ "system": "http://snomed.info/sct", "code": "247472004", "display": "Hives" }],
                        "text": "Hives"
                    }],
                    "onset": "2020-03-01",
                    "severity": "moderate"
                }],
                "patient": { "reference": fhir_out::urn_for("doc", "Patient", "patient-1") }
            })
        );
        assert!(report.issues.is_empty(), "{:?}", codes(&report));
    }

    #[test]
    fn type_and_category_follow_the_ig_maps() {
        for (code, ty, cat) in [
            ("235719002", Some("intolerance"), Some("food")),
            ("414285001", Some("allergy"), Some("food")),
            ("418471000", None, Some("food")),
            ("416098002", Some("allergy"), Some("medication")),
            ("419511003", None, Some("medication")),
            ("59037007", Some("intolerance"), Some("medication")),
            ("419199007", Some("allergy"), None),
            ("418038007", None, None),
            ("420134006", None, None),
        ] {
            let (out, _) = run(&document(
                ACTIVE,
                &observation("", "", code, penicillin(), ""),
            ));
            assert_eq!(out[0].get("type").and_then(Value::as_str), ty, "{code}");
            let cat_actual = out[0]["category"][0].as_str();
            assert_eq!(cat_actual, cat, "{code}");
        }
    }

    #[test]
    fn status_precedence_and_defaults() {
        let status = |code: &str| {
            format!(
                r#"<entryRelationship typeCode="REFR" inversionInd="true"><observation classCode="OBS" moodCode="EVN">
  <code code="33999-4" codeSystem="2.16.840.1.113883.6.1"/>
  <value xsi:type="CD" code="{code}" codeSystem="2.16.840.1.113883.6.96"/>
</observation></entryRelationship>"#
            )
        };
        let get = |concern: &str, extras: &str| {
            let (out, report) = run(&document(
                concern,
                &observation("", "", "416098002", penicillin(), extras),
            ));
            (
                out[0]["clinicalStatus"]["coding"][0]["code"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
                codes(&report),
            )
        };
        assert_eq!(get(ACTIVE, &status("413322009")).0, "resolved");
        assert_eq!(get(ACTIVE, &status("73425007")).0, "inactive");
        assert_eq!(get(r#"<statusCode code="completed"/>"#, "").0, "inactive");
        assert_eq!(get("", ""), ("active".to_owned(), vec![]));
        let (s, c) = get(ACTIVE, &status("999"));
        assert_eq!((s.as_str(), c), ("active", vec!["allergy.status.unmapped"]));
    }

    #[test]
    fn negated_with_null_substance_uses_the_no_known_allergy_map() {
        let null_substance = r#"<participant typeCode="CSM"><participantRole><playingEntity><code nullFlavor="NA"/></playingEntity></participantRole></participant>"#;
        for (value, code) in [
            ("414285001", "429625007"),
            ("416098002", "409137002"),
            ("419199007", "716186003"),
        ] {
            let (out, report) = run(&document(
                ACTIVE,
                &observation(r#"negationInd="true""#, "", value, null_substance, ""),
            ));
            assert_eq!(
                out[0]["code"],
                json!({ "coding": [{ "system": "http://snomed.info/sct", "code": code }] })
            );
            assert!(report.issues.is_empty());
        }
        let (out, report) = run(&document(
            ACTIVE,
            &observation(r#"negationInd="true""#, "", "235719002", null_substance, ""),
        ));
        assert_eq!(out[0]["code"], json!({ "text": "no known allergies" }));
        assert_eq!(codes(&report), ["allergy.negated.no-known-code"]);
    }

    #[test]
    fn negated_with_a_coded_substance_uses_the_exposure_risk_extension() {
        let (out, report) = run(&document(
            ACTIVE,
            &observation(r#"negationInd="true""#, "", "416098002", penicillin(), ""),
        ));
        assert!(out[0].get("code").is_none());
        let ext = &out[0]["extension"][0];
        assert_eq!(ext["url"], EXPOSURE_RISK_URL);
        assert_eq!(ext["extension"][0]["url"], "substance");
        assert_eq!(
            ext["extension"][0]["valueCodeableConcept"]["coding"][0]["code"],
            "7980"
        );
        assert_eq!(
            ext["extension"][1]["valueCodeableConcept"]["coding"][0],
            json!({ "system": EXPOSURE_RISK_SYSTEM, "code": "no-known-reaction-risk" })
        );
        assert_eq!(codes(&report), ["allergy.negated.substance-exposure-risk"]);
    }

    #[test]
    fn reaction_severity_wins_over_allergy_severity_which_is_inherited_otherwise() {
        let two = format!(
            "{}{}{}",
            reaction(HIVES, &severity("24484000")),
            reaction(HIVES, ""),
            severity("255604002")
        );
        let (out, report) = run(&document(
            ACTIVE,
            &observation("", "", "416098002", penicillin(), &two),
        ));
        assert_eq!(out[0]["reaction"][0]["severity"], "severe");
        assert_eq!(out[0]["reaction"][1]["severity"], "mild");
        assert!(report.issues.is_empty());
    }

    #[test]
    fn severity_without_a_reaction_is_reported_and_dropped() {
        let (out, report) = run(&document(
            ACTIVE,
            &observation("", "", "416098002", penicillin(), &severity("6736007")),
        ));
        assert!(out[0].get("reaction").is_none());
        assert_eq!(codes(&report), ["allergy.severity.no-reaction"]);
    }

    #[test]
    fn reaction_without_a_value_is_omitted_with_a_warning() {
        let (out, report) = run(&document(
            ACTIVE,
            &observation("", "", "416098002", penicillin(), &reaction("", "")),
        ));
        assert!(out[0].get("reaction").is_none());
        assert_eq!(codes(&report), ["allergy.reaction.no-manifestation"]);
    }

    #[test]
    fn end_date_becomes_the_abatement_extension() {
        let (out, _) = run(&document(
            ACTIVE,
            &observation(
                "",
                r#"<effectiveTime><low value="2010"/><high value="20150605"/></effectiveTime>"#,
                "416098002",
                penicillin(),
                "",
            ),
        ));
        assert_eq!(out[0]["onsetDateTime"], "2010");
        assert_eq!(
            out[0]["extension"],
            json!([{ "url": ABATEMENT_URL, "valueDateTime": "2015-06-05" }])
        );
    }

    #[test]
    fn missing_allergen_is_reported() {
        let (out, report) = run(&document(ACTIVE, &observation("", "", "416098002", "", "")));
        assert!(out[0].get("code").is_none());
        assert_eq!(codes(&report), ["allergy.code.missing"]);
    }

    #[test]
    fn concern_without_observation_makes_nothing() {
        let (out, report) = run(&document(ACTIVE, ""));
        assert!(out.is_empty());
        assert_eq!(codes(&report), ["allergy.concern.no-observation"]);
    }

    #[test]
    fn every_ig_map_value_is_covered() {
        for (c, s) in [
            ("255604002", "mild"),
            ("6736007", "moderate"),
            ("24484000", "severe"),
        ] {
            assert_eq!(severity_of(c), Some(s));
        }
        for (c, s) in [
            ("CRITH", "high"),
            ("CRITL", "low"),
            ("CRITU", "unable-to-assess"),
        ] {
            assert_eq!(criticality_of(c), Some(s));
        }
        for (c, s) in [
            ("413322009", "resolved"),
            ("73425007", "inactive"),
            ("55561003", "active"),
        ] {
            assert_eq!(status_of(c), Some(s));
        }
    }
}
