//! Medication Activity (`substanceAdministration`) to FHIR `MedicationRequest`.
//!
//! Rules come from the IG page "CF-medications", its CSV and the ConceptMap `CF-MedicationStatus`
//! (HL7/ccda-on-fhir). Where we deliberately differ from the IG, the choice is recorded in
//! `docs/decisions/0005-medications.md`.

use ccda_model::{ClinicalDocument, Quantity, Statement, StatementKind, Time, Value as Cv};
use serde_json::{json, Map, Value};

use crate::author::{apply_authors, register_organization, register_practitioner, AuthorFields};
use crate::util::nested;
use crate::{
    data_absent_reason_extension, map_codeable_concept, map_datetime, map_identifier, map_quantity,
    Context,
};

const MEDICATION_ACTIVITY: &str = "2.16.840.1.113883.10.20.22.4.16";
const SUPPLY_ORDER: &str = "2.16.840.1.113883.10.20.22.4.17";
const MEDICATION_DISPENSE: &str = "2.16.840.1.113883.10.20.22.4.18";
const INDICATION: &str = "2.16.840.1.113883.10.20.22.4.19";
const INSTRUCTION: &str = "2.16.840.1.113883.10.20.22.4.20";
const FREE_TEXT_SIG: &str = "2.16.840.1.113883.10.20.22.4.147";

const FIELDS: AuthorFields = AuthorFields {
    recorder: Some("requester"),
    date: "authoredOn",
};

/// Map every Medication Activity in the document.
pub fn map_medications(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind != StatementKind::SubstanceAdministration
                || !s.has_template(MEDICATION_ACTIVITY)
            {
                continue;
            }
            match s.mood_code.as_deref() {
                Some("EVN" | "INT") | None => out.push(map_activity(s, ctx)),
                Some(_) => ctx.report.info(
                    "medication.mood.unsupported",
                    "a medication with a mood other than EVN or INT was not mapped",
                ),
            }
        }
    }
    out
}

/// `CF-MedicationStatus`: v3 ActStatus to medicationrequest-status.
fn status_of(code: &str) -> Option<&'static str> {
    Some(match code {
        "active" => "active",
        "suspended" => "on-hold",
        "aborted" => "stopped",
        "completed" => "completed",
        "nullified" => "entered-in-error",
        "cancelled" => "cancelled",
        _ => return None,
    })
}

fn dar(nf: &str, ctx: &mut Context) -> Value {
    json!({ "extension": [data_absent_reason_extension(nf, ctx.report)] })
}

/// A UCUM unit FHIR's `Timing.repeat.periodUnit` accepts.
fn period_unit(unit: &str) -> Option<&'static str> {
    Some(match unit {
        "s" => "s",
        "min" => "min",
        "h" => "h",
        "d" => "d",
        "wk" => "wk",
        "mo" => "mo",
        "a" => "a",
        _ => return None,
    })
}

fn number(q: &Quantity) -> Option<serde_json::Number> {
    q.value.as_deref().and_then(crate::decimal)
}

/// Build `Dosage.timing` from the statement's effective times and repeat number.
fn timing(s: &Statement, ctx: &mut Context) -> Option<Value> {
    let mut repeat = Map::new();
    let mut event: Option<String> = None;

    for time in &s.effective_times {
        match time.xsi_type.as_deref() {
            Some("PIVL_TS") => period(time, &mut repeat, ctx),
            Some("EIVL_TS") => when(time, &mut repeat, ctx),
            Some("SXPR_TS" | "PIVL_PPD_TS" | "SXCM_TS" | "PPD_TS") => ctx.report.info(
                "medication.timing.unsupported",
                "an effectiveTime of a complex type was not mapped to a timing",
            ),
            _ => {
                if let Some(raw) = &time.value {
                    event = map_datetime(raw, ctx.report);
                }
                let mut bounds = Map::new();
                for (key, point) in [("start", &time.low), ("end", &time.high)] {
                    if let Some(date) = point
                        .as_ref()
                        .and_then(|p| p.value.as_deref())
                        .and_then(|raw| map_datetime(raw, ctx.report))
                    {
                        bounds.insert(key.into(), json!(date));
                    }
                }
                if !bounds.is_empty() {
                    repeat.insert("boundsPeriod".into(), Value::Object(bounds));
                }
            }
        }
    }

    if let Some(raw) = s.repeat_number.as_deref() {
        if s.mood_code.as_deref() == Some("INT") {
            match raw.parse::<u32>().ok().filter(|n| *n > 0) {
                Some(n) => {
                    repeat.insert("count".into(), json!(n));
                }
                None => ctx.report.warn(
                    "medication.repeat-number.invalid",
                    "repeatNumber is not a positive integer; the count was omitted",
                ),
            }
        } else {
            ctx.report.info(
                "medication.repeat-number.unsupported",
                "repeatNumber on a medication event has no FHIR element (IG: not supported); it was omitted",
            );
        }
    }

    let mut timing = Map::new();
    if let Some(event) = event {
        timing.insert("event".into(), json!([event]));
    }
    if !repeat.is_empty() {
        timing.insert("repeat".into(), Value::Object(repeat));
    }
    (!timing.is_empty()).then_some(Value::Object(timing))
}

/// `PIVL_TS`: a repeat period, which may be a range ("every 4 to 6 hours").
fn period(time: &Time, repeat: &mut Map<String, Value>, ctx: &mut Context) {
    let Some(p) = &time.period else {
        return;
    };
    let (low, high, unit) = match (&p.value, &p.low) {
        (Some(v), _) => (crate::decimal(v), None, p.unit.as_deref()),
        (None, Some(l)) => (
            number(l),
            p.high.as_ref().and_then(number),
            l.unit.as_deref().or(p.unit.as_deref()),
        ),
        (None, None) => return,
    };
    let (Some(low), Some(unit)) = (low, unit) else {
        return;
    };
    let Some(unit) = period_unit(unit) else {
        ctx.report.warn(
            "medication.period.unit",
            "a dosing period uses a unit FHIR timing does not allow and was omitted",
        );
        return;
    };
    repeat.insert("period".into(), Value::Number(low));
    if let Some(high) = high {
        repeat.insert("periodMax".into(), Value::Number(high));
    }
    repeat.insert("periodUnit".into(), json!(unit));
}

/// `EIVL_TS`: an event such as before meals, with an optional offset in minutes.
fn when(time: &Time, repeat: &mut Map<String, Value>, ctx: &mut Context) {
    if let Some(code) = &time.event {
        repeat.insert("when".into(), json!([code]));
    }
    if let Some(offset) = &time.offset {
        match (offset.unit.as_deref(), number(offset)) {
            (Some("min"), Some(n)) if n.is_u64() => {
                repeat.insert("offset".into(), Value::Number(n));
            }
            _ => ctx.report.warn(
                "medication.offset.unsupported",
                "an event offset is not a whole number of minutes and was omitted",
            ),
        }
    }
}

fn ratio(parts: &(Option<Quantity>, Option<Quantity>), ctx: &mut Context) -> Option<Value> {
    let numerator = map_quantity(parts.0.as_ref()?, ctx.report)?;
    let denominator = map_quantity(parts.1.as_ref()?, ctx.report)?;
    Some(json!({ "numerator": numerator, "denominator": denominator }))
}

fn dosage(s: &Statement, ctx: &mut Context) -> Option<Value> {
    let mut d = Map::new();

    // The free text sig is exactly `Dosage.text`; an Instruction is patient-facing.
    for related in nested(s) {
        if related.has_template(FREE_TEXT_SIG) {
            if let Some(text) = related.text.as_ref().filter(|t| !t.trim().is_empty()) {
                d.insert("text".into(), json!(text));
            }
        } else if related.has_template(INSTRUCTION) {
            if let Some(text) = related.text.as_ref().filter(|t| !t.trim().is_empty()) {
                d.insert("patientInstruction".into(), json!(text));
            } else if let Some(c) = related
                .code
                .as_ref()
                .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
            {
                if let Some(t) = c["text"].as_str() {
                    d.insert("patientInstruction".into(), json!(t));
                }
            }
        }
    }

    if let Some(t) = timing(s, ctx) {
        d.insert("timing".into(), t);
    }
    if s.has_precondition {
        d.insert("asNeededBoolean".into(), json!(true));
        ctx.report.info(
            "medication.precondition.as-needed-assumed",
            "a precondition was read as 'take as needed'; its criterion was not mapped",
        );
    }
    if let Some(route) = s
        .route_code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
    {
        d.insert("route".into(), route);
    }

    let mut rate = Map::new();
    if let Some(q) = s
        .dose_quantity
        .as_ref()
        .and_then(|q| map_quantity(q, ctx.report))
    {
        rate.insert("doseQuantity".into(), q);
    }
    if let Some((low, high)) = &s.dose_range {
        let mut range = Map::new();
        for (key, q) in [("low", low), ("high", high)] {
            if let Some(v) = q.as_ref().and_then(|q| map_quantity(q, ctx.report)) {
                range.insert(key.into(), v);
            }
        }
        if !range.is_empty() {
            rate.insert("doseRange".into(), Value::Object(range));
        }
    }
    if let Some(q) = s
        .rate_quantity
        .as_ref()
        .and_then(|q| map_quantity(q, ctx.report))
    {
        rate.insert("rateQuantity".into(), q);
    }
    if !rate.is_empty() {
        d.insert("doseAndRate".into(), json!([rate]));
    }
    if let Some(max) = s.max_dose_quantity.as_ref().and_then(|m| ratio(m, ctx)) {
        d.insert("maxDosePerPeriod".into(), max);
    }
    if s.administration_unit_code.is_some() {
        ctx.report.info(
            "medication.administration-unit.unsupported",
            "administrationUnitCode has no MedicationRequest element (IG: not supported); it was omitted",
        );
    }
    (!d.is_empty()).then_some(Value::Object(d))
}

fn map_activity(s: &Statement, ctx: &mut Context) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("MedicationRequest"));
    r.insert("id".into(), json!(ctx.next_id("medicationrequest")));

    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }

    let status = match s.status_code.as_deref() {
        Some(code) => status_of(code).unwrap_or_else(|| {
            ctx.report.warn(
                "medication.status.unmapped",
                "a statusCode is not in the IG's status map; status was set to unknown",
            );
            "unknown"
        }),
        None => {
            ctx.report.warn(
                "medication.status.missing",
                "a medication has no statusCode; status was set to unknown",
            );
            "unknown"
        }
    };
    r.insert("status".into(), json!(status));
    // Per the IG: INT is an order, EVN a plan.
    let intent = if s.mood_code.as_deref() == Some("EVN") {
        "plan"
    } else {
        "order"
    };
    r.insert("intent".into(), json!(intent));
    if s.negation_ind {
        r.insert("doNotPerform".into(), json!(true));
        ctx.report.info(
            "medication.negation",
            "a negated medication was mapped with doNotPerform set",
        );
    }

    // The drug: a plain code, or a Medication resource when there is a manufacturer.
    let code = s.consumable.as_ref().and_then(|c| c.code.as_ref());
    let concept = code.and_then(|c| {
        map_codeable_concept(c, ctx.terminology, ctx.report)
            .or_else(|| c.null_flavor.as_deref().map(|nf| dar(nf, ctx)))
    });
    let manufacturer = s
        .consumable
        .as_ref()
        .and_then(|c| c.manufacturer.as_ref())
        .and_then(|org| register_organization(org, ctx));
    match (concept, manufacturer) {
        (Some(concept), Some(manufacturer)) => {
            let id = ctx.next_id("medication");
            ctx.push_extra(json!({
                "resourceType": "Medication",
                "id": id,
                "code": concept,
                "manufacturer": { "reference": manufacturer },
            }));
            r.insert(
                "medicationReference".into(),
                json!({ "reference": ctx.reference("Medication", &id) }),
            );
        }
        (Some(concept), None) => {
            r.insert("medicationCodeableConcept".into(), concept);
        }
        (None, _) => {
            ctx.report.warn(
                "medication.drug.missing",
                "a medication has no drug code; the medication was marked unknown",
            );
            r.insert("medicationCodeableConcept".into(), dar("UNK", ctx));
        }
    }
    r.insert("subject".into(), json!({ "reference": ctx.patient }));

    if let Some(d) = dosage(s, ctx) {
        r.insert("dosageInstruction".into(), json!([d]));
    }

    // R4 allows one performer.
    let performers: Vec<String> = s
        .performers
        .iter()
        .filter_map(|p| {
            register_practitioner(p, ctx).or_else(|| {
                p.organization
                    .as_ref()
                    .and_then(|org| register_organization(org, ctx))
            })
        })
        .collect();
    if let Some(first) = performers.first() {
        r.insert("performer".into(), json!({ "reference": first }));
        if performers.iter().any(|p| p != first) {
            ctx.report.info(
                "medication.performer.multiple",
                "MedicationRequest allows one performer; the first was kept",
            );
        }
    }

    let mut reasons: Vec<Value> = Vec::new();
    for related in nested(s) {
        if related.has_template(INDICATION) {
            if let Some(Cv::Coded(code)) = related.values.first() {
                if let Some(c) = map_codeable_concept(code, ctx.terminology, ctx.report) {
                    reasons.push(c);
                }
            }
        } else if related.has_template(SUPPLY_ORDER) || related.has_template(MEDICATION_DISPENSE) {
            ctx.report.info(
                "medication.supply.unsupported",
                "a supply order or dispense under a medication was not mapped yet",
            );
        }
    }
    if !reasons.is_empty() {
        r.insert("reasonCode".into(), json!(reasons));
    }
    if s.participants
        .iter()
        .any(|p| p.type_code.as_deref() == Some("CSM"))
    {
        ctx.report.info(
            "medication.vehicle.unsupported",
            "a drug vehicle participant has no MedicationRequest element (IG: not supported); it was omitted",
        );
    }

    let authors: Vec<_> = s.authors.iter().collect();
    apply_authors(&mut r, FIELDS, &authors, ctx);
    Value::Object(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn document(mood: &str, attrs: &str, body: &str) -> String {
        format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section>
  <code code="10160-0" codeSystem="2.16.840.1.113883.6.1"/>
  <entry><substanceAdministration classCode="SBADM" moodCode="{mood}" {attrs}>
    <templateId root="2.16.840.1.113883.10.20.22.4.16"/>
    <id root="1.2.3" extension="MED-1"/>
    {body}
  </substanceAdministration></entry>
</section></component></structuredBody></component></ClinicalDocument>"#
        )
    }

    const DRUG: &str = r#"<consumable><manufacturedProduct>
  <manufacturedMaterial><code code="314076" codeSystem="2.16.840.1.113883.6.88"/></manufacturedMaterial>
</manufacturedProduct></consumable>"#;

    fn run(xml: &str) -> (Vec<Value>, Vec<Value>, Report) {
        let doc = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_medications(&doc, &mut ctx);
        let extra = ctx.take_extra();
        (out, extra, report)
    }

    fn codes(report: &Report) -> Vec<&'static str> {
        report.issues.iter().map(|i| i.code).collect()
    }

    #[test]
    fn status_map_follows_the_ig() {
        for (code, want) in [
            ("active", "active"),
            ("suspended", "on-hold"),
            ("aborted", "stopped"),
            ("completed", "completed"),
            ("nullified", "entered-in-error"),
            ("cancelled", "cancelled"),
        ] {
            let xml = document("EVN", "", &format!(r#"<statusCode code="{code}"/>{DRUG}"#));
            let (out, _, _) = run(&xml);
            assert_eq!(out[0]["status"], want, "{code}");
        }
    }

    #[test]
    fn unknown_or_missing_status_is_unknown_and_reported() {
        let (out, _, report) = run(&document(
            "EVN",
            "",
            &format!(r#"<statusCode code="new"/>{DRUG}"#),
        ));
        assert_eq!(out[0]["status"], "unknown");
        assert!(codes(&report).contains(&"medication.status.unmapped"));
        let (out, _, report) = run(&document("EVN", "", DRUG));
        assert_eq!(out[0]["status"], "unknown");
        assert!(codes(&report).contains(&"medication.status.missing"));
    }

    #[test]
    fn intent_follows_mood() {
        let (out, _, _) = run(&document("INT", "", DRUG));
        assert_eq!(out[0]["intent"], "order");
        let (out, _, _) = run(&document("EVN", "", DRUG));
        assert_eq!(out[0]["intent"], "plan");
    }

    #[test]
    fn other_moods_are_not_mapped() {
        let (out, _, report) = run(&document("RQO", "", DRUG));
        assert!(out.is_empty());
        assert!(codes(&report).contains(&"medication.mood.unsupported"));
    }

    #[test]
    fn negation_sets_do_not_perform() {
        let (out, _, report) = run(&document("EVN", r#"negationInd="true""#, DRUG));
        assert_eq!(out[0]["doNotPerform"], true);
        assert!(codes(&report).contains(&"medication.negation"));
    }

    #[test]
    fn missing_drug_is_marked_unknown() {
        let (out, _, report) = run(&document("EVN", "", ""));
        assert_eq!(
            out[0]["medicationCodeableConcept"]["extension"][0]["valueCode"],
            "unknown"
        );
        assert!(codes(&report).contains(&"medication.drug.missing"));
    }

    #[test]
    fn manufacturer_makes_a_medication_resource() {
        let body = r#"<consumable><manufacturedProduct>
  <manufacturedMaterial><code code="314076" codeSystem="2.16.840.1.113883.6.88"/></manufacturedMaterial>
  <manufacturerOrganization><name>Maker</name></manufacturerOrganization>
</manufacturedProduct></consumable>"#;
        let (out, extra, _) = run(&document("EVN", "", body));
        assert!(out[0]["medicationReference"]["reference"].is_string());
        assert!(out[0].get("medicationCodeableConcept").is_none());
        let med = extra
            .iter()
            .find(|r| r["resourceType"] == "Medication")
            .unwrap();
        assert_eq!(med["code"]["coding"][0]["code"], "314076");
        assert!(med["manufacturer"]["reference"].is_string());
    }

    #[test]
    fn period_and_range() {
        let body = format!(
            r#"<statusCode code="active"/>
<effectiveTime xsi:type="PIVL_TS"><period value="8" unit="h"/></effectiveTime>{DRUG}"#
        );
        let (out, _, _) = run(&document("EVN", "", &body));
        let repeat = &out[0]["dosageInstruction"][0]["timing"]["repeat"];
        assert_eq!(repeat["period"], 8);
        assert_eq!(repeat["periodUnit"], "h");
        assert!(repeat.get("periodMax").is_none());
    }

    #[test]
    fn unknown_period_unit_is_dropped_with_a_warning() {
        let body = format!(
            r#"<effectiveTime xsi:type="PIVL_TS"><period value="1" unit="fortnight"/></effectiveTime>{DRUG}"#
        );
        let (out, _, report) = run(&document("EVN", "", &body));
        assert!(out[0].get("dosageInstruction").is_none());
        assert!(codes(&report).contains(&"medication.period.unit"));
    }

    #[test]
    fn repeat_number_is_a_count_for_orders_only() {
        let body = format!(r#"<repeatNumber value="3"/>{DRUG}"#);
        let (out, _, _) = run(&document("INT", "", &body));
        assert_eq!(
            out[0]["dosageInstruction"][0]["timing"]["repeat"]["count"],
            3
        );
        let (out, _, report) = run(&document("EVN", "", &body));
        assert!(out[0].get("dosageInstruction").is_none());
        assert!(codes(&report).contains(&"medication.repeat-number.unsupported"));
        let bad = format!(r#"<repeatNumber value="x"/>{DRUG}"#);
        let (_, _, report) = run(&document("INT", "", &bad));
        assert!(codes(&report).contains(&"medication.repeat-number.invalid"));
    }

    #[test]
    fn offset_must_be_whole_minutes() {
        let body = format!(
            r#"<effectiveTime xsi:type="EIVL_TS"><event code="PC"/><offset value="1" unit="h"/></effectiveTime>{DRUG}"#
        );
        let (out, _, report) = run(&document("EVN", "", &body));
        let repeat = &out[0]["dosageInstruction"][0]["timing"]["repeat"];
        assert_eq!(repeat["when"][0], "PC");
        assert!(repeat.get("offset").is_none());
        assert!(codes(&report).contains(&"medication.offset.unsupported"));
    }

    #[test]
    fn exact_decimals_are_kept() {
        let body = format!(r#"<doseQuantity value="0.50" unit="mg"/>{DRUG}"#);
        let (out, _, _) = run(&document("EVN", "", &body));
        let text = serde_json::to_string(&out[0]).unwrap();
        assert!(text.contains("0.50"));
    }

    #[test]
    fn messages_never_contain_document_values() {
        let body = format!(
            r#"<statusCode code="SecretStatus"/><precondition><criterion/></precondition>
<effectiveTime xsi:type="PIVL_TS"><period value="1" unit="SecretUnit"/></effectiveTime>{DRUG}"#
        );
        let (_, _, report) = run(&document("EVN", "", &body));
        for issue in &report.issues {
            assert!(!issue.message.contains("Secret"), "{}", issue.message);
        }
    }
}
