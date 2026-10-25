//! Immunization Activity (`substanceAdministration`, mood EVN) to FHIR `Immunization`.
//!
//! Rules come from the IG page "CF-immunizations", its CSV and the ConceptMap
//! `CF-ImmunizationStatus` (HL7/ccda-on-fhir). Planned or proposed immunizations (mood INT) are
//! `MedicationRequest`s in the IG and are reported, not yet mapped.

use ccda_model::{ClinicalDocument, Statement, StatementKind, Value as Cv};
use serde_json::{json, Map, Value};

use crate::author::{apply_authors, register_organization, register_practitioner, AuthorFields};
use crate::util::{concept, nested};
use crate::{
    data_absent_reason_extension, map_codeable_concept, map_datetime, map_quantity, Context,
};

const IMMUNIZATION_ACTIVITY: &str = "2.16.840.1.113883.10.20.22.4.52";
const NOT_GIVEN_REASON: &str = "2.16.840.1.113883.10.20.22.4.53";
const INDICATION: &str = "2.16.840.1.113883.10.20.22.4.19";
const REACTION: &str = "2.16.840.1.113883.10.20.22.4.9";

const PERFORMER_FUNCTION_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/v2-0443";

/// `Immunization.recorded` holds the earliest author time; there is no `recorder` element.
const FIELDS: AuthorFields = AuthorFields {
    recorder: None,
    date: "recorded",
};

/// Map every Immunization Activity in the document.
pub fn map_immunizations(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind != StatementKind::SubstanceAdministration
                || !s.has_template(IMMUNIZATION_ACTIVITY)
            {
                continue;
            }
            match s.mood_code.as_deref() {
                Some("EVN") | None => out.push(map_activity(s, ctx)),
                Some(_) => ctx.report.info(
                    "immunization.intent.unsupported",
                    "a planned or proposed immunization (not an event) was not mapped; the IG maps it to MedicationRequest",
                ),
            }
        }
    }
    out
}

/// `CF-ImmunizationStatus` for a non-negated activity: v3 ActStatus to event-status.
fn status_of(code: &str) -> Option<&'static str> {
    Some(match code {
        "completed" => "completed",
        "nullified" => "entered-in-error",
        "aborted" | "cancelled" | "held" | "new" | "obsolete" | "suspended" => "not-done",
        _ => return None,
    })
}

fn status(s: &Statement, ctx: &mut Context) -> &'static str {
    if s.negation_ind {
        return "not-done";
    }
    match s.status_code.as_deref() {
        None => "completed",
        Some(code) => status_of(code).unwrap_or_else(|| {
            ctx.report.warn(
                "immunization.status.unmapped",
                "a statusCode is not in the IG's status map; status defaulted to completed",
            );
            "completed"
        }),
    }
}

fn dar(nf: &str, ctx: &mut Context) -> Value {
    json!({ "extension": [data_absent_reason_extension(nf, ctx.report)] })
}

fn map_activity(s: &Statement, ctx: &mut Context) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Immunization"));
    r.insert("id".into(), json!(ctx.next_id("immunization")));

    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|id| crate::map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }

    r.insert("status".into(), json!(status(s, ctx)));

    // Vaccine (required): consumable/manufacturedProduct/manufacturedMaterial/code.
    let material = s.consumable.as_ref().and_then(|c| c.code.as_ref());
    let vaccine = match material {
        Some(code) => map_codeable_concept(code, ctx.terminology, ctx.report)
            .or_else(|| code.null_flavor.as_deref().map(|nf| dar(nf, ctx))),
        None => None,
    };
    r.insert(
        "vaccineCode".into(),
        vaccine.unwrap_or_else(|| {
            ctx.report.warn(
                "immunization.vaccine.missing",
                "an Immunization Activity has no vaccine code; vaccineCode was marked unknown",
            );
            dar("UNK", ctx)
        }),
    );
    r.insert("patient".into(), json!({ "reference": ctx.patient }));

    // Occurrence (required): the point value, or the start of an interval.
    let time = s.effective_time.as_ref();
    let point = time.and_then(|t| {
        t.value
            .clone()
            .or_else(|| t.low.as_ref().and_then(|l| l.value.clone()))
    });
    match point {
        Some(raw) => match map_datetime(&raw, ctx.report) {
            Some(date) => {
                r.insert("occurrenceDateTime".into(), json!(date));
            }
            None => {
                r.insert("_occurrenceDateTime".into(), dar("UNK", ctx));
            }
        },
        None => {
            let nf = time
                .and_then(|t| {
                    t.null_flavor
                        .clone()
                        .or_else(|| t.low.as_ref().and_then(|l| l.null_flavor.clone()))
                })
                .unwrap_or_else(|| {
                    ctx.report.warn(
                        "immunization.occurrence.missing",
                        "an immunization has no date; occurrence was marked unknown",
                    );
                    "UNK".to_owned()
                });
            r.insert("_occurrenceDateTime".into(), dar(&nf, ctx));
        }
    }

    if let Some(raw) = s.repeat_number.as_deref() {
        match raw.parse::<u32>().ok().filter(|n| *n > 0) {
            Some(n) => {
                r.insert(
                    "protocolApplied".into(),
                    json!([{ "doseNumberPositiveInt": n }]),
                );
            }
            None => ctx.report.warn(
                "immunization.dose-number.invalid",
                "repeatNumber is not a positive integer; the dose number was omitted",
            ),
        }
    }

    for (field, code) in [("route", &s.route_code), ("site", &s.approach_site_code)] {
        if let Some(c) = code
            .as_ref()
            .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
        {
            r.insert(field.into(), c);
        }
    }

    if let Some(quantity) = s
        .dose_quantity
        .as_ref()
        .and_then(|q| map_quantity(q, ctx.report))
    {
        r.insert("doseQuantity".into(), quantity);
    }
    if s.dose_range.is_some() {
        ctx.report.info(
            "immunization.dose.range-unsupported",
            "a dose given as a range has no Immunization element (IG: not supported); it was omitted",
        );
    }

    if let Some(consumable) = &s.consumable {
        if let Some(lot) = &consumable.lot_number {
            r.insert("lotNumber".into(), json!(lot));
        }
        if let Some(org) = &consumable.manufacturer {
            if let Some(reference) = register_organization(org, ctx) {
                r.insert("manufacturer".into(), json!({ "reference": reference }));
            }
        }
    }

    // Performers: administering provider.
    let mut performers: Vec<Value> = Vec::new();
    for performer in &s.performers {
        let actor = register_practitioner(performer, ctx).or_else(|| {
            performer
                .organization
                .as_ref()
                .and_then(|org| register_organization(org, ctx))
        });
        if let Some(reference) = actor {
            let entry = json!({
                "function": concept(PERFORMER_FUNCTION_SYSTEM, "AP"),
                "actor": { "reference": reference }
            });
            if !performers.contains(&entry) {
                performers.push(entry);
            }
        }
    }
    if !performers.is_empty() {
        r.insert("performer".into(), json!(performers));
    }

    // Reasons: why it was not given, and why it was given.
    let mut reasons: Vec<Value> = Vec::new();
    for related in nested(s) {
        if related.has_template(NOT_GIVEN_REASON) {
            if let Some(c) = related
                .code
                .as_ref()
                .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
            {
                r.insert("statusReason".into(), c);
            }
        } else if related.has_template(INDICATION) {
            if let Some(Cv::Coded(code)) = related.values.first() {
                if let Some(c) = map_codeable_concept(code, ctx.terminology, ctx.report) {
                    reasons.push(c);
                }
            }
        } else if related.has_template(REACTION) {
            ctx.report.info(
                "immunization.reaction.unsupported",
                "a reaction to an immunization is not mapped yet",
            );
        }
    }

    if !reasons.is_empty() {
        r.insert("reasonCode".into(), json!(reasons));
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

    /// One Immunization Activity with the given attributes and body.
    fn document(attrs: &str, body: &str) -> String {
        format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section>
  <code code="11369-6" codeSystem="2.16.840.1.113883.6.1"/>
  <entry><substanceAdministration classCode="SBADM" moodCode="EVN" {attrs}>
    <templateId root="2.16.840.1.113883.10.20.22.4.52"/>
    <id root="1.2.3" extension="IMM-1"/>
    {body}
  </substanceAdministration></entry>
</section></component></structuredBody></component></ClinicalDocument>"#
        )
    }

    const FLU: &str = r#"<consumable><manufacturedProduct classCode="MANU">
  <templateId root="2.16.840.1.113883.10.20.22.4.54"/>
  <manufacturedMaterial>
    <code code="140" codeSystem="2.16.840.1.113883.12.292" displayName="Influenza, seasonal, injectable, preservative free">
      <translation code="49281-0623" codeSystem="2.16.840.1.113883.6.69"/>
    </code>
    <lotNumberText>LOT-42</lotNumberText>
  </manufacturedMaterial>
  <manufacturerOrganization><name>Maker Inc</name></manufacturerOrganization>
</manufacturedProduct></consumable>"#;

    fn run(xml: &str) -> (Vec<Value>, Vec<Value>, Report) {
        let doc = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_immunizations(&doc, &mut ctx);
        let extra = ctx.take_extra();
        (out, extra, report)
    }

    fn codes(report: &Report) -> Vec<&'static str> {
        report.issues.iter().map(|i| i.code).collect()
    }

    #[test]
    fn completed_immunization_follows_the_ig() {
        let body = format!(
            r#"<statusCode code="completed"/>
<effectiveTime value="20100815"/>
<repeatNumber value="1"/>
<routeCode code="C28161" codeSystem="2.16.840.1.113883.3.26.1.1" displayName="Intramuscular Route of Administration"/>
<approachSiteCode code="368208006" codeSystem="2.16.840.1.113883.6.96"/>
<doseQuantity value="0.5" unit="mL"/>
{FLU}
<performer><assignedEntity>
  <id root="2.16.840.1.113883.4.6" extension="1234567893"/>
  <assignedPerson><name><given>Pat</given><family>Doe</family></name></assignedPerson>
</assignedEntity></performer>"#
        );
        let (out, extra, report) = run(&document("", &body));
        assert_eq!(out.len(), 1);
        let i = &out[0];
        assert_eq!(i["resourceType"], "Immunization");
        assert_eq!(i["id"], "immunization-1");
        assert_eq!(
            i["identifier"][0],
            json!({ "system": "urn:oid:1.2.3", "value": "IMM-1" })
        );
        assert_eq!(i["status"], "completed");
        assert_eq!(i["occurrenceDateTime"], "2010-08-15");
        assert_eq!(
            i["protocolApplied"],
            json!([{ "doseNumberPositiveInt": 1 }])
        );
        assert_eq!(
            i["vaccineCode"]["coding"][0]["system"],
            "http://hl7.org/fhir/sid/cvx"
        );
        assert_eq!(i["vaccineCode"]["coding"][0]["code"], "140");
        assert_eq!(
            i["vaccineCode"]["coding"][1]["system"],
            "http://hl7.org/fhir/sid/ndc"
        );
        assert_eq!(
            i["route"]["coding"][0]["system"],
            "http://ncicb.nci.nih.gov/xml/owl/EVS/Thesaurus.owl"
        );
        assert_eq!(i["site"]["coding"][0]["code"], "368208006");
        assert_eq!(i["doseQuantity"]["value"].to_string(), "0.5");
        assert_eq!(i["doseQuantity"]["unit"], "mL");
        assert_eq!(i["lotNumber"], "LOT-42");
        assert_eq!(
            i["patient"]["reference"],
            fhir_out::urn_for("doc", "Patient", "patient-1")
        );
        assert_eq!(i["performer"][0]["function"]["coding"][0]["code"], "AP");
        assert_eq!(
            i["performer"][0]["actor"]["reference"],
            fhir_out::urn_for("doc", "Practitioner", "practitioner-1")
        );
        let manufacturer = &i["manufacturer"]["reference"];
        assert_eq!(
            *manufacturer,
            json!(fhir_out::urn_for("doc", "Organization", "organization-1"))
        );
        assert_eq!(
            extra
                .iter()
                .filter(|r| r["resourceType"] == "Practitioner")
                .count(),
            1
        );
        assert_eq!(
            extra
                .iter()
                .filter(|r| r["resourceType"] == "Organization")
                .count(),
            1
        );
        assert_eq!(codes(&report), ["organization.active.assumed"]);
    }

    #[test]
    fn status_follows_negation_then_the_ig_map() {
        let get = |attrs: &str, status: &str| {
            let body = format!("{status}<effectiveTime value=\"2020\"/>{FLU}");
            run(&document(attrs, &body)).0[0]["status"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        assert_eq!(
            get(r#"negationInd="true""#, r#"<statusCode code="completed"/>"#),
            "not-done"
        );
        assert_eq!(get(r#"negationInd="false""#, ""), "completed");
        assert_eq!(get("", r#"<statusCode code="completed"/>"#), "completed");
        assert_eq!(
            get("", r#"<statusCode code="nullified"/>"#),
            "entered-in-error"
        );
        for s in [
            "aborted",
            "cancelled",
            "held",
            "new",
            "obsolete",
            "suspended",
        ] {
            assert_eq!(
                get("", &format!(r#"<statusCode code="{s}"/>"#)),
                "not-done",
                "{s}"
            );
        }
    }

    #[test]
    fn unknown_status_warns_and_defaults_to_completed() {
        let (out, _, report) = run(&document(
            "",
            &format!(r#"<statusCode code="zzz"/><effectiveTime value="2020"/>{FLU}"#),
        ));
        assert_eq!(out[0]["status"], "completed");
        assert_eq!(codes(&report)[0], "immunization.status.unmapped");
        assert!(!report.issues[0].message.contains("zzz"));
    }

    #[test]
    fn occurrence_uses_low_of_an_interval_or_a_data_absent_reason() {
        let (out, _, _) = run(&document(
            "",
            &format!(
                r#"<effectiveTime><low value="20200102"/><high value="20200103"/></effectiveTime>{FLU}"#
            ),
        ));
        assert_eq!(out[0]["occurrenceDateTime"], "2020-01-02");

        let (out, _, report) = run(&document("", FLU));
        assert_eq!(
            out[0]["_occurrenceDateTime"]["extension"][0]["valueCode"],
            "unknown"
        );
        assert!(codes(&report).contains(&"immunization.occurrence.missing"));

        let (out, _, report) = run(&document(
            "",
            &format!(r#"<effectiveTime nullFlavor="NA"/>{FLU}"#),
        ));
        assert_eq!(
            out[0]["_occurrenceDateTime"]["extension"][0]["valueCode"],
            "not-applicable"
        );
        assert!(!codes(&report).contains(&"immunization.occurrence.missing"));
    }

    #[test]
    fn missing_vaccine_is_marked_unknown_with_a_warning() {
        let (out, _, report) = run(&document("", r#"<effectiveTime value="2020"/>"#));
        assert_eq!(
            out[0]["vaccineCode"]["extension"][0]["valueCode"],
            "unknown"
        );
        assert_eq!(codes(&report), ["immunization.vaccine.missing"]);
    }

    #[test]
    fn invalid_dose_number_is_reported() {
        for bad in ["0", "-1", "x", "1.5"] {
            let (out, _, report) = run(&document(
                "",
                &format!(r#"<repeatNumber value="{bad}"/><effectiveTime value="2020"/>{FLU}"#),
            ));
            assert!(out[0].get("protocolApplied").is_none(), "{bad}");
            assert!(codes(&report).contains(&"immunization.dose-number.invalid"));
        }
    }

    #[test]
    fn reasons_split_by_template() {
        let body = format!(
            r#"<effectiveTime value="2020"/>{FLU}
<entryRelationship typeCode="RSON"><observation classCode="OBS" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.53"/>
  <code code="MEDPREC" codeSystem="2.16.840.1.113883.5.8" displayName="medical precaution"/>
</observation></entryRelationship>
<entryRelationship typeCode="RSON"><observation classCode="OBS" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.19"/>
  <code code="404684003" codeSystem="2.16.840.1.113883.6.96"/>
  <value xsi:type="CD" code="195967001" codeSystem="2.16.840.1.113883.6.96"/>
</observation></entryRelationship>"#
        );
        let (out, _, _) = run(&document(r#"negationInd="true""#, &body));
        assert_eq!(out[0]["statusReason"]["coding"][0]["code"], "MEDPREC");
        assert_eq!(out[0]["reasonCode"][0]["coding"][0]["code"], "195967001");
    }

    #[test]
    fn planned_immunizations_are_reported_not_mapped() {
        let xml = document("", FLU).replace(r#"moodCode="EVN""#, r#"moodCode="INT""#);
        let (out, _, report) = run(&xml);
        assert!(out.is_empty());
        assert_eq!(codes(&report), ["immunization.intent.unsupported"]);
    }

    #[test]
    fn authors_set_recorded_and_create_provenance() {
        let body = format!(
            r#"<effectiveTime value="2020"/>{FLU}
<author><time value="20200105080000-0500"/><assignedAuthor>
  <id root="2.16.840.1.113883.4.6" extension="1234567893"/>
  <assignedPerson><name><given>Pat</given><family>Doe</family></name></assignedPerson>
</assignedAuthor></author>"#
        );
        let (out, extra, _) = run(&document("", &body));
        assert_eq!(out[0]["recorded"], "2020-01-05T08:00:00-05:00");
        assert!(out[0].get("recordedDate").is_none());
        assert!(out[0].get("recorder").is_none());
        assert!(extra.iter().any(|r| r["resourceType"] == "Provenance"));
    }

    #[test]
    fn dose_range_and_reaction_are_reported() {
        let body = format!(
            r#"<effectiveTime value="2020"/><doseQuantity><low value="1" unit="mL"/><high value="2" unit="mL"/></doseQuantity>{FLU}
<entryRelationship typeCode="MFST" inversionInd="true"><observation classCode="OBS" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.9"/>
</observation></entryRelationship>"#
        );
        let (out, _, report) = run(&document("", &body));
        assert!(out[0].get("doseQuantity").is_none());
        let c = codes(&report);
        assert!(c.contains(&"immunization.dose.range-unsupported"));
        assert!(c.contains(&"immunization.reaction.unsupported"));
    }
}
