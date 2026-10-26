//! Problem Concern Acts to FHIR `Condition`.
//!
//! Rules come from the IG page "CF-problems" and its ConceptMaps `CF-ProblemCategory`,
//! `CF-ProblemStatus` and `CF-ConditionClinicalStatus` (HL7/ccda-on-fhir). Each Problem
//! Observation nested in a Problem Concern Act becomes its own `Condition`.

use ccda_model::{ClinicalDocument, Section, Statement, Value as Cv};
use serde_json::{json, Map, Value};

use crate::author::{apply_authors, RECORDER_FIELDS};
use crate::util::{concept, is_coded, LOINC, SNOMED};
use crate::{
    data_absent_reason_extension, map_age, map_codeable_concept, map_datetime, map_identifier,
    Context,
};

const CONCERN_ACT: &str = "2.16.840.1.113883.10.20.22.4.3";
/// Risk Concern Act: a concern act that wraps Problem Observations, like the Problem Concern Act.
const RISK_CONCERN_ACT: &str = "2.16.840.1.113883.10.20.22.4.136";
const PROBLEM_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.4";

const CLINICAL_STATUS_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/condition-clinical";
const VERIFICATION_STATUS_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/condition-ver-status";
const CATEGORY_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/condition-category";
const US_CORE_CATEGORY_SYSTEM: &str = "http://hl7.org/fhir/us/core/CodeSystem/condition-category";
const ASSERTED_DATE_URL: &str = "http://hl7.org/fhir/StructureDefinition/condition-assertedDate";

/// Map every Problem Concern Act in the document to `Condition` resources.
pub fn map_problems(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            if entry.statement.has_template(CONCERN_ACT)
                || entry.statement.has_template(RISK_CONCERN_ACT)
            {
                map_concern(section, &entry.statement, ctx, &mut out);
            } else if DIAGNOSIS_WRAPPERS
                .iter()
                .any(|t| entry.statement.has_template(t))
            {
                out.extend(map_diagnosis_wrapper(&entry.statement, ctx));
            }
        }
    }
    out
}

/// Hospital Admission (4.34) and Discharge (4.33) Diagnosis, Preprocedure (4.65) and
/// Postprocedure (4.51) Diagnosis: acts that wrap Problem Observations.
const DIAGNOSIS_WRAPPERS: &[&str] = &[
    "2.16.840.1.113883.10.20.22.4.33",
    "2.16.840.1.113883.10.20.22.4.34",
    "2.16.840.1.113883.10.20.22.4.51",
    "2.16.840.1.113883.10.20.22.4.65",
];

/// The Conditions inside a diagnosis wrapper: `encounter-diagnosis`, plus the wrapper's own code
/// (admission, discharge, pre- or post-procedure) as a second category so the role is kept.
fn map_diagnosis_wrapper(act: &Statement, ctx: &mut Context) -> Vec<Value> {
    let observations: Vec<_> = act
        .related
        .iter()
        .map(|r| &r.statement)
        .filter(|s| s.has_template(PROBLEM_OBSERVATION))
        .collect();
    if observations.is_empty() {
        ctx.report.warn(
            "condition.concern.no-observation",
            "a diagnosis act has no Problem Observation; no Condition was created",
        );
    }
    let role = act
        .code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report));
    observations
        .into_iter()
        .map(|observation| {
            let base =
                json!({ "coding": [{ "system": CATEGORY_SYSTEM, "code": "encounter-diagnosis" }] });
            let mut condition = map_observation(Some(base), act, observation, ctx);
            if let Some(role) = role.clone() {
                if let Some(first) = condition.get("category").cloned() {
                    let mut categories = match first {
                        Value::Array(a) => a,
                        other => vec![other],
                    };
                    categories.push(role);
                    condition["category"] = json!(categories);
                }
            }
            condition
        })
        .collect()
}

fn map_concern(section: &Section, concern: &Statement, ctx: &mut Context, out: &mut Vec<Value>) {
    let observations: Vec<_> = concern
        .related_by("SUBJ")
        .filter(|s| s.has_template(PROBLEM_OBSERVATION))
        .collect();
    if observations.is_empty() {
        ctx.report.warn(
            "condition.concern.no-observation",
            "a Problem Concern Act has no Problem Observation; no Condition was created",
        );
    }
    let category = category(section, ctx);
    for observation in observations {
        out.push(map_observation(category.clone(), concern, observation, ctx));
    }
}

/// The Conditions of an Encounter Diagnosis act nested in an encounter. They map like problems
/// but are always `encounter-diagnosis`, and point back at the encounter.
pub(crate) fn map_encounter_diagnosis(
    act: &Statement,
    encounter_reference: &str,
    ctx: &mut Context,
) -> Vec<Value> {
    let category =
        json!({ "coding": [{ "system": CATEGORY_SYSTEM, "code": "encounter-diagnosis" }] });
    act.related
        .iter()
        .map(|r| &r.statement)
        .filter(|s| s.has_template(PROBLEM_OBSERVATION))
        .map(|observation| {
            let mut condition = map_observation(Some(category.clone()), act, observation, ctx);
            condition["encounter"] = json!({ "reference": encounter_reference });
            condition
        })
        .collect()
}

/// `CF-ProblemCategory`: the section a problem sits in decides its category.
fn category(section: &Section, ctx: &mut Context) -> Option<Value> {
    let code = section
        .code
        .as_ref()
        .filter(|c| c.code_system.as_deref() == Some(LOINC))
        .and_then(|c| c.code.as_deref());
    let (system, code) = match code {
        Some("11450-4") => (CATEGORY_SYSTEM, "problem-list-item"),
        Some("46240-8") => (CATEGORY_SYSTEM, "encounter-diagnosis"),
        Some("75310-3") => (US_CORE_CATEGORY_SYSTEM, "health-concern"),
        _ => {
            ctx.report.info(
                "condition.category.unmapped-section",
                "a problem sits in a section the IG has no category for; category was omitted",
            );
            return None;
        }
    };
    Some(json!({ "coding": [{ "system": system, "code": code }] }))
}

/// `CF-ProblemStatus`: SNOMED problem status to `condition-clinical`.
fn status_from_problem_status(code: &str) -> Option<&'static str> {
    Some(match code {
        "246455001" => "recurrence",
        "263855007" => "relapse",
        "277022003" => "remission",
        "413322009" => "resolved",
        "55561003" => "active",
        "73425007" => "inactive",
        _ => return None,
    })
}

/// `CF-ConditionClinicalStatus`: v3 ActStatus of the concern to `condition-clinical`.
fn status_from_act_status(code: &str) -> Option<&'static str> {
    Some(match code {
        "active" => "active",
        "aborted" | "completed" | "suspended" => "inactive",
        _ => return None,
    })
}

/// The status from the Problem Status Observation (LOINC 33999-4), if there is a usable one.
fn problem_status(observation: &Statement, ctx: &mut Context) -> Option<&'static str> {
    let status = observation
        .related
        .iter()
        .map(|r| &r.statement)
        .find(|s| is_coded(s, "33999-4", LOINC))?;
    let Some(Cv::Coded(value)) = status.values.first() else {
        return None;
    };
    let mapped = value
        .code_system
        .as_deref()
        .filter(|oid| *oid == SNOMED)
        .and(value.code.as_deref())
        .and_then(status_from_problem_status);
    if mapped.is_none() && value.null_flavor.is_none() {
        ctx.report.warn(
            "condition.status.unmapped",
            "a Problem Status value is not in the IG's status map; the concern status was used instead",
        );
    }
    mapped
}

fn clinical_status(
    observation: &Statement,
    concern: &Statement,
    ctx: &mut Context,
) -> &'static str {
    if let Some(status) = problem_status(observation, ctx) {
        return status;
    }
    if let Some(code) = concern.status_code.as_deref() {
        if let Some(status) = status_from_act_status(code) {
            return status;
        }
        ctx.report.warn(
            "condition.status.unmapped",
            "a concern statusCode is not in the IG's status map; clinicalStatus defaulted to active",
        );
        return "active";
    }
    ctx.report.info(
        "condition.status.defaulted",
        "no problem status or concern statusCode; clinicalStatus defaulted to active as the IG directs",
    );
    "active"
}

pub(crate) fn map_observation(
    category: Option<Value>,
    concern: &Statement,
    observation: &Statement,
    ctx: &mut Context,
) -> Value {
    let mut resource = Map::new();
    resource.insert("resourceType".into(), json!("Condition"));
    resource.insert("id".into(), json!(ctx.next_id("condition")));

    let identifiers: Vec<Value> = observation
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        resource.insert("identifier".into(), json!(identifiers));
    }

    // Clinical status, adjusted if the problem has an end date (FHIR invariant con-4).
    let time = observation.effective_time.as_ref();
    let high = time.and_then(|t| t.high.as_ref());
    let abated = high.is_some_and(|h| h.value.is_some() || h.null_flavor.as_deref() == Some("UNK"));
    let mut status = clinical_status(observation, concern, ctx);
    if abated && matches!(status, "active" | "recurrence" | "relapse") {
        ctx.report.warn(
            "condition.status.adjusted-for-abatement",
            "a problem has an end date but an active status; clinicalStatus was set to inactive",
        );
        status = "inactive";
    }
    resource.insert(
        "clinicalStatus".into(),
        concept(CLINICAL_STATUS_SYSTEM, status),
    );

    if observation.negation_ind {
        ctx.report.info(
            "condition.negated",
            "a problem is negated; verificationStatus was set to refuted",
        );
        resource.insert(
            "verificationStatus".into(),
            concept(VERIFICATION_STATUS_SYSTEM, "refuted"),
        );
    }

    if let Some(category) = category {
        resource.insert("category".into(), json!([category]));
    }

    match observation.values.first() {
        Some(Cv::Coded(code)) => {
            if let Some(code_concept) = map_codeable_concept(code, ctx.terminology, ctx.report) {
                resource.insert("code".into(), code_concept);
            } else if let Some(nf) = code.null_flavor.as_deref() {
                let ext = data_absent_reason_extension(nf, ctx.report);
                resource.insert("code".into(), json!({ "extension": [ext] }));
            } else {
                ctx.report.warn(
                    "condition.code.missing",
                    "a Problem Observation value has no usable code; code was omitted",
                );
            }
        }
        Some(_) => ctx.report.warn(
            "condition.code.not-coded",
            "a Problem Observation value is not a coded value; code was omitted",
        ),
        None => ctx.report.warn(
            "condition.code.missing",
            "a Problem Observation has no value; code was omitted",
        ),
    }

    // Onset and abatement.
    let onset = time.and_then(|t| {
        t.low.clone().or_else(|| {
            t.value.clone().map(|value| ccda_model::TimePoint {
                value: Some(value),
                null_flavor: None,
            })
        })
    });
    let mut has_onset_date = false;
    if let Some(point) = onset {
        if let Some(raw) = point.value.as_deref() {
            if let Some(date) = map_datetime(raw, ctx.report) {
                resource.insert("onsetDateTime".into(), json!(date));
                has_onset_date = true;
            }
        } else if let Some(nf) = point.null_flavor.as_deref() {
            let ext = data_absent_reason_extension(nf, ctx.report);
            resource.insert("_onsetDateTime".into(), json!({ "extension": [ext] }));
            has_onset_date = true;
        }
    }
    if let Some(point) = high {
        if let Some(raw) = point.value.as_deref() {
            if let Some(date) = map_datetime(raw, ctx.report) {
                resource.insert("abatementDateTime".into(), json!(date));
            }
        } else if point.null_flavor.as_deref() == Some("UNK") {
            // The IG: UNK on `high` is how C-CDA says "resolved, date unknown".
            let ext = data_absent_reason_extension("UNK", ctx.report);
            resource.insert("_abatementDateTime".into(), json!({ "extension": [ext] }));
        }
    }

    // Age at onset (Age Observation, SNOMED 445518008). Only one onset[x] may be present.
    if let Some(age) = observation
        .related
        .iter()
        .map(|r| &r.statement)
        .find(|s| is_coded(s, "445518008", SNOMED))
    {
        if let Some(Cv::Quantity(quantity)) = age.values.first() {
            if has_onset_date {
                ctx.report.info(
                    "condition.onset.age-ignored",
                    "a problem has both an onset date and an age at onset; only the date was kept",
                );
            } else if let Some(value) = map_age(quantity, ctx.report) {
                resource.insert("onsetAge".into(), value);
            }
        }
    }

    // Date of Diagnosis (LOINC 77975-1) becomes the assertedDate extension.
    if let Some(diagnosis) = observation
        .related
        .iter()
        .map(|r| &r.statement)
        .find(|s| is_coded(s, "77975-1", LOINC))
    {
        let raw = diagnosis.effective_time.as_ref().and_then(|t| {
            t.value
                .as_deref()
                .or_else(|| t.low.as_ref().and_then(|l| l.value.as_deref()))
        });
        if let Some(date) = raw.and_then(|r| map_datetime(r, ctx.report)) {
            resource.insert(
                "extension".into(),
                json!([{ "url": ASSERTED_DATE_URL, "valueDateTime": date }]),
            );
        }
    }

    // Supporting observations become Observations that `evidence.detail` points at (IG).
    let mut details: Vec<Value> = Vec::new();
    for supporting in observation.related_by("SPRT") {
        if supporting.kind != ccda_model::StatementKind::Observation {
            continue;
        }
        let mut o = crate::observation::base(supporting, "evidence", ctx);
        crate::observation::apply_performer(&mut o, supporting, None, ctx);
        let id = o["id"].as_str().unwrap_or_default().to_owned();
        details.push(json!({ "reference": ctx.reference("Observation", &id) }));
        ctx.push_extra(Value::Object(o));
    }
    if !details.is_empty() {
        resource.insert("evidence".into(), json!([{ "detail": details }]));
    }

    resource.insert("subject".into(), json!({ "reference": ctx.patient }));
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

    const SNOMED_OID: &str = "2.16.840.1.113883.6.96";

    /// A document with one section (LOINC `section_code`) and one concern act.
    fn document(section_code: &str, concern_status: &str, observations: &str) -> String {
        format!(
            r##"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section>
  <code code="{section_code}" codeSystem="2.16.840.1.113883.6.1"/>
  <text><paragraph ID="p1">Essential hypertension</paragraph></text>
  <entry><act classCode="ACT" moodCode="EVN">
    <templateId root="2.16.840.1.113883.10.20.22.4.3"/>
    <id root="1.2.3" extension="CONCERN"/>
    <code code="CONC" codeSystem="2.16.840.1.113883.5.6"/>
    {concern_status}
    {observations}
  </act></entry>
</section></component></structuredBody></component></ClinicalDocument>"##
        )
    }

    /// A Problem Observation with the given extras inside.
    fn observation(attrs: &str, time: &str, value: &str, extras: &str) -> String {
        format!(
            r#"<entryRelationship typeCode="SUBJ"><observation classCode="OBS" moodCode="EVN" {attrs}>
  <templateId root="2.16.840.1.113883.10.20.22.4.4"/>
  <id root="1.2.3" extension="OBS-1"/>
  <code code="55607006" codeSystem="{SNOMED_OID}"/>
  {time}
  {value}
  {extras}
</observation></entryRelationship>"#
        )
    }

    fn hypertension() -> &'static str {
        r##"<value xsi:type="CD" code="59621000" codeSystem="2.16.840.1.113883.6.96" displayName="Essential hypertension"><originalText><reference value="#p1"/></originalText></value>"##
    }

    fn status_obs(code: &str) -> String {
        format!(
            r#"<entryRelationship typeCode="REFR"><observation classCode="OBS" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.6"/>
  <code code="33999-4" codeSystem="2.16.840.1.113883.6.1"/>
  <value xsi:type="CD" code="{code}" codeSystem="{SNOMED_OID}"/>
</observation></entryRelationship>"#
        )
    }

    fn run(xml: &str) -> (Vec<Value>, Report) {
        let doc = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let conditions = map_problems(&doc, &mut ctx);
        (conditions, report)
    }

    fn codes(report: &Report) -> Vec<&'static str> {
        report.issues.iter().map(|i| i.code).collect()
    }

    #[test]
    fn standard_problem_follows_the_ig_example() {
        let xml = document(
            "11450-4",
            r#"<statusCode code="active"/>"#,
            &observation(
                "",
                r#"<effectiveTime><low value="20120806"/></effectiveTime>"#,
                hypertension(),
                "",
            ),
        );
        let (conditions, report) = run(&xml);
        assert_eq!(conditions.len(), 1);
        assert_eq!(
            conditions[0],
            json!({
                "resourceType": "Condition",
                "id": "condition-1",
                "identifier": [{ "system": "urn:oid:1.2.3", "value": "OBS-1" }],
                "clinicalStatus": { "coding": [{ "system": CLINICAL_STATUS_SYSTEM, "code": "active" }] },
                "category": [{ "coding": [{ "system": CATEGORY_SYSTEM, "code": "problem-list-item" }] }],
                "code": {
                    "coding": [{ "system": "http://snomed.info/sct", "code": "59621000", "display": "Essential hypertension" }],
                    "text": "Essential hypertension"
                },
                "onsetDateTime": "2012-08-06",
                "subject": { "reference": fhir_out::urn_for("doc", "Patient", "patient-1") }
            })
        );
        assert!(report.issues.is_empty(), "{:?}", codes(&report));
    }

    #[test]
    fn problem_status_observation_wins_over_concern_status() {
        let xml = document(
            "11450-4",
            r#"<statusCode code="active"/>"#,
            &observation(
                "",
                r#"<effectiveTime><low value="2012"/><high value="20130101"/></effectiveTime>"#,
                hypertension(),
                &status_obs("413322009"),
            ),
        );
        let (conditions, report) = run(&xml);
        assert_eq!(
            conditions[0]["clinicalStatus"]["coding"][0]["code"],
            "resolved"
        );
        assert_eq!(conditions[0]["onsetDateTime"], "2012");
        assert_eq!(conditions[0]["abatementDateTime"], "2013-01-01");
        assert!(report.issues.is_empty(), "{:?}", codes(&report));
    }

    #[test]
    fn every_problem_status_in_the_ig_map() {
        for (snomed, fhir) in [
            ("246455001", "recurrence"),
            ("263855007", "relapse"),
            ("277022003", "remission"),
            ("413322009", "resolved"),
            ("55561003", "active"),
            ("73425007", "inactive"),
        ] {
            assert_eq!(status_from_problem_status(snomed), Some(fhir));
        }
        for (act, fhir) in [
            ("active", "active"),
            ("aborted", "inactive"),
            ("completed", "inactive"),
            ("suspended", "inactive"),
        ] {
            assert_eq!(status_from_act_status(act), Some(fhir));
        }
        assert_eq!(status_from_act_status("nullified"), None);
    }

    #[test]
    fn concern_status_is_used_when_there_is_no_problem_status() {
        let xml = document(
            "11450-4",
            r#"<statusCode code="completed"/>"#,
            &observation("", "", hypertension(), ""),
        );
        let (conditions, _) = run(&xml);
        assert_eq!(
            conditions[0]["clinicalStatus"]["coding"][0]["code"],
            "inactive"
        );
    }

    #[test]
    fn no_status_anywhere_defaults_to_active_and_says_so() {
        let xml = document("11450-4", "", &observation("", "", hypertension(), ""));
        let (conditions, report) = run(&xml);
        assert_eq!(
            conditions[0]["clinicalStatus"]["coding"][0]["code"],
            "active"
        );
        assert_eq!(codes(&report), ["condition.status.defaulted"]);
    }

    #[test]
    fn unmapped_statuses_warn_without_values() {
        let xml = document(
            "11450-4",
            r#"<statusCode code="nullified"/>"#,
            &observation("", "", hypertension(), &status_obs("999999")),
        );
        let (conditions, report) = run(&xml);
        assert_eq!(
            conditions[0]["clinicalStatus"]["coding"][0]["code"],
            "active"
        );
        assert_eq!(
            codes(&report),
            ["condition.status.unmapped", "condition.status.unmapped"]
        );
        assert!(report
            .issues
            .iter()
            .all(|i| !i.message.contains("999999") && !i.message.contains("nullified")));
    }

    #[test]
    fn end_date_forces_a_conformant_status() {
        let xml = document(
            "11450-4",
            r#"<statusCode code="active"/>"#,
            &observation(
                "",
                r#"<effectiveTime><low value="2012"/><high value="20130101"/></effectiveTime>"#,
                hypertension(),
                "",
            ),
        );
        let (conditions, report) = run(&xml);
        assert_eq!(
            conditions[0]["clinicalStatus"]["coding"][0]["code"],
            "inactive"
        );
        assert_eq!(codes(&report), ["condition.status.adjusted-for-abatement"]);
    }

    #[test]
    fn unknown_end_date_becomes_a_data_absent_reason() {
        let xml = document(
            "11450-4",
            r#"<statusCode code="completed"/>"#,
            &observation(
                "",
                r#"<effectiveTime><low value="2012"/><high nullFlavor="UNK"/></effectiveTime>"#,
                hypertension(),
                &status_obs("413322009"),
            ),
        );
        let (conditions, report) = run(&xml);
        assert_eq!(
            conditions[0]["_abatementDateTime"],
            json!({ "extension": [{ "url": DAR_URL, "valueCode": "unknown" }] })
        );
        assert!(conditions[0].get("abatementDateTime").is_none());
        assert_eq!(
            conditions[0]["clinicalStatus"]["coding"][0]["code"],
            "resolved"
        );
        assert!(report.issues.is_empty());
    }

    const DAR_URL: &str = "http://hl7.org/fhir/StructureDefinition/data-absent-reason";

    #[test]
    fn unknown_onset_becomes_a_data_absent_reason() {
        let xml = document(
            "11450-4",
            r#"<statusCode code="active"/>"#,
            &observation(
                "",
                r#"<effectiveTime><low nullFlavor="UNK"/></effectiveTime>"#,
                hypertension(),
                "",
            ),
        );
        let (conditions, _) = run(&xml);
        assert_eq!(
            conditions[0]["_onsetDateTime"]["extension"][0]["valueCode"],
            "unknown"
        );
    }

    #[test]
    fn negation_means_refuted() {
        let xml = document(
            "11450-4",
            r#"<statusCode code="active"/>"#,
            &observation(r#"negationInd="true""#, "", hypertension(), ""),
        );
        let (conditions, report) = run(&xml);
        assert_eq!(
            conditions[0]["verificationStatus"]["coding"][0],
            json!({ "system": VERIFICATION_STATUS_SYSTEM, "code": "refuted" })
        );
        assert_eq!(codes(&report), ["condition.negated"]);
    }

    #[test]
    fn category_follows_the_section_code() {
        for (section, system, code) in [
            ("11450-4", CATEGORY_SYSTEM, "problem-list-item"),
            ("46240-8", CATEGORY_SYSTEM, "encounter-diagnosis"),
            ("75310-3", US_CORE_CATEGORY_SYSTEM, "health-concern"),
        ] {
            let xml = document(
                section,
                r#"<statusCode code="active"/>"#,
                &observation("", "", hypertension(), ""),
            );
            let (conditions, _) = run(&xml);
            assert_eq!(
                conditions[0]["category"][0]["coding"][0],
                json!({ "system": system, "code": code }),
                "{section}"
            );
        }
    }

    #[test]
    fn unknown_section_omits_category_with_a_note() {
        let xml = document(
            "10160-0",
            r#"<statusCode code="active"/>"#,
            &observation("", "", hypertension(), ""),
        );
        let (conditions, report) = run(&xml);
        assert!(conditions[0].get("category").is_none());
        assert_eq!(codes(&report), ["condition.category.unmapped-section"]);
    }

    #[test]
    fn age_at_onset_is_used_only_without_an_onset_date() {
        let age = r#"<entryRelationship typeCode="SUBJ" inversionInd="true"><observation classCode="OBS" moodCode="EVN">
  <code code="445518008" codeSystem="2.16.840.1.113883.6.96"/>
  <value xsi:type="PQ" value="45" unit="a"/>
</observation></entryRelationship>"#;
        let status = r#"<statusCode code="active"/>"#;

        let (conditions, report) = run(&document(
            "11450-4",
            status,
            &observation("", "", hypertension(), age),
        ));
        assert_eq!(
            conditions[0]["onsetAge"],
            json!({ "value": 45, "unit": "a", "system": "http://unitsofmeasure.org", "code": "a" })
        );
        assert!(report.issues.is_empty());

        let with_date = observation(
            "",
            r#"<effectiveTime><low value="2012"/></effectiveTime>"#,
            hypertension(),
            age,
        );
        let (conditions, report) = run(&document("11450-4", status, &with_date));
        assert!(conditions[0].get("onsetAge").is_none());
        assert_eq!(conditions[0]["onsetDateTime"], "2012");
        assert_eq!(codes(&report), ["condition.onset.age-ignored"]);
    }

    #[test]
    fn date_of_diagnosis_becomes_asserted_date() {
        let diagnosis = r#"<entryRelationship typeCode="COMP"><act classCode="ACT" moodCode="EVN">
  <code code="77975-1" codeSystem="2.16.840.1.113883.6.1"/>
  <effectiveTime value="20120901"/>
</act></entryRelationship>"#;
        let (conditions, _) = run(&document(
            "11450-4",
            r#"<statusCode code="active"/>"#,
            &observation("", "", hypertension(), diagnosis),
        ));
        assert_eq!(
            conditions[0]["extension"],
            json!([{ "url": ASSERTED_DATE_URL, "valueDateTime": "2012-09-01" }])
        );
    }

    #[test]
    fn missing_or_empty_values_are_reported_not_fatal() {
        let status = r#"<statusCode code="active"/>"#;

        let (c, r) = run(&document("11450-4", status, &observation("", "", "", "")));
        assert!(c[0].get("code").is_none());
        assert_eq!(codes(&r), ["condition.code.missing"]);

        let null_value = r#"<value xsi:type="CD" nullFlavor="UNK"/>"#;
        let (c, r) = run(&document(
            "11450-4",
            status,
            &observation("", "", null_value, ""),
        ));
        assert_eq!(c[0]["code"]["extension"][0]["valueCode"], "unknown");
        assert!(r.issues.is_empty());

        let wrong_type = r#"<value xsi:type="PQ" value="1" unit="a"/>"#;
        let (c, r) = run(&document(
            "11450-4",
            status,
            &observation("", "", wrong_type, ""),
        ));
        assert!(c[0].get("code").is_none());
        assert_eq!(codes(&r), ["condition.code.not-coded"]);
    }

    #[test]
    fn concern_without_observation_makes_no_condition() {
        let (conditions, report) = run(&document("11450-4", "", ""));
        assert!(conditions.is_empty());
        assert_eq!(codes(&report), ["condition.concern.no-observation"]);
    }

    #[test]
    fn each_observation_becomes_its_own_condition() {
        let two = format!(
            "{}{}",
            observation("", "", hypertension(), ""),
            observation("", "", hypertension(), "")
        );
        let (conditions, _) = run(&document("11450-4", r#"<statusCode code="active"/>"#, &two));
        let ids: Vec<_> = conditions
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["condition-1", "condition-2"]);
    }

    #[test]
    fn supporting_observation_becomes_evidence_detail() {
        let sprt = r#"<entryRelationship typeCode="SPRT"><observation classCode="OBS" moodCode="EVN">
  <id root="1.2.3" extension="EV-1"/>
  <code code="44261-6" codeSystem="2.16.840.1.113883.6.1"/>
  <statusCode code="completed"/>
  <value xsi:type="INT" value="12"/>
</observation></entryRelationship>"#;
        let xml = document(
            "11450-4",
            r#"<statusCode code="active"/>"#,
            &observation("", "", hypertension(), sprt),
        );
        let doc = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let conditions = map_problems(&doc, &mut ctx);
        let extra = ctx.take_extra();
        let reference = conditions[0]["evidence"][0]["detail"][0]["reference"]
            .as_str()
            .unwrap();
        assert!(reference.starts_with("urn:uuid:"));
        let obs = extra
            .iter()
            .find(|r| r["resourceType"] == "Observation")
            .unwrap();
        assert_eq!(obs["code"]["coding"][0]["code"], "44261-6");
        assert_eq!(obs["valueInteger"], 12);
    }

    #[test]
    fn output_is_deterministic() {
        let xml = document(
            "11450-4",
            r#"<statusCode code="active"/>"#,
            &observation("", "", hypertension(), ""),
        );
        assert_eq!(run(&xml).0, run(&xml).0);
    }

    #[test]
    fn admission_diagnosis_keeps_its_role_as_a_second_category() {
        let xml = r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section><entry><act classCode="ACT" moodCode="EVN">
<templateId root="2.16.840.1.113883.10.20.22.4.34"/>
<code code="46241-6" codeSystem="2.16.840.1.113883.6.1"/>
<entryRelationship typeCode="REFR"><observation classCode="OBS" moodCode="EVN">
<templateId root="2.16.840.1.113883.10.20.22.4.4"/>
<code code="282291009" codeSystem="2.16.840.1.113883.6.96"/><statusCode code="completed"/>
<value xsi:type="CD" code="233604007" codeSystem="2.16.840.1.113883.6.96"/>
</observation></entryRelationship></act></entry></section></component></structuredBody></component></ClinicalDocument>"#;
        let (conditions, _) = run(xml);
        assert_eq!(conditions.len(), 1);
        assert_eq!(
            conditions[0]["category"][0]["coding"][0]["code"],
            "encounter-diagnosis"
        );
        assert_eq!(conditions[0]["category"][1]["coding"][0]["code"], "46241-6");
    }
}
