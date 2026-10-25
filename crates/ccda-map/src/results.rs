//! Result Organizer and Result Observations to FHIR `DiagnosticReport` and `Observation`.
//!
//! Rules come from the IG page "CF-results". One organizer becomes one `DiagnosticReport` that
//! lists, in `result`, one `Observation` per Result Observation.
//!
//! The IG chooses the category from the LOINC class of the code, which needs a LOINC database we
//! do not have yet. Until then every result in a Results section is treated as laboratory and the
//! assumption is reported (`results.category.assumed-laboratory`).

use ccda_model::{ClinicalDocument, Statement, StatementKind};
use serde_json::{json, Map, Value};

use crate::observation::{apply_performer, base, category, insert_effective, insert_span, status};
use crate::{
    data_absent_reason_extension, map_codeable_concept, map_datetime, map_identifier, Context,
};

const RESULT_ORGANIZER: &str = "2.16.840.1.113883.10.20.22.4.1";
const RESULT_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.2";
const V2_0074: &str = "http://terminology.hl7.org/CodeSystem/v2-0074";

/// Map every result organizer (and any loose Result Observation) in the document.
pub fn map_results(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind == StatementKind::Organizer && s.has_template(RESULT_ORGANIZER) {
                report(s, ctx, &mut out);
            } else if s.kind == StatementKind::Observation && s.has_template(RESULT_OBSERVATION) {
                out.push(Value::Object(observation(s, None, ctx)));
            }
        }
    }
    out
}

fn observation(s: &Statement, org: Option<&Statement>, ctx: &mut Context) -> Map<String, Value> {
    let mut r = base(s, "result", ctx);
    r.insert(
        "category".into(),
        category_of(ctx, "laboratory", "Laboratory"),
    );
    if !s.participants.is_empty() {
        ctx.report.info(
            "results.specimen.unsupported",
            "a specimen or other participant on a result was not mapped yet",
        );
    }
    apply_performer(&mut r, s, org, ctx);
    r
}

fn category_of(ctx: &mut Context, code: &str, display: &str) -> Value {
    ctx.report.info(
        "results.category.assumed-laboratory",
        "a result was given the laboratory category without checking the LOINC class",
    );
    category(code, display)
}

fn report(org: &Statement, ctx: &mut Context, out: &mut Vec<Value>) {
    let members: Vec<Value> = org
        .related
        .iter()
        .map(|r| &r.statement)
        .filter(|m| m.kind == StatementKind::Observation && m.has_template(RESULT_OBSERVATION))
        .map(|m| Value::Object(observation(m, Some(org), ctx)))
        .collect();

    let mut r = Map::new();
    r.insert("resourceType".into(), json!("DiagnosticReport"));
    r.insert("id".into(), json!(ctx.next_id("diagnosticreport")));
    let identifiers: Vec<Value> = org
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    r.insert("status".into(), json!(status(org, ctx)));
    ctx.report.info(
        "results.category.assumed-laboratory",
        "a result was given the laboratory category without checking the LOINC class",
    );
    r.insert(
        "category".into(),
        json!([{ "coding": [{ "system": V2_0074, "code": "LAB", "display": "Laboratory" }] }]),
    );
    let code = org.code.as_ref().and_then(|c| {
        map_codeable_concept(c, ctx.terminology, ctx.report).or_else(|| {
            c.null_flavor
                .as_deref()
                .map(|nf| json!({ "extension": [data_absent_reason_extension(nf, ctx.report)] }))
        })
    });
    r.insert(
        "code".into(),
        code.unwrap_or_else(|| {
            ctx.report.warn(
                "results.code.missing",
                "a result organizer has no code; code was marked unknown",
            );
            json!({ "extension": [data_absent_reason_extension("UNK", ctx.report)] })
        }),
    );
    r.insert("subject".into(), json!({ "reference": ctx.patient }));
    insert_effective(&mut r, org, ctx);
    if !r.contains_key("effectiveDateTime") && !r.contains_key("effectivePeriod") {
        insert_span(&mut r, &members);
    }
    // `issued` is an instant. The report has no such time in C-CDA, so use the latest author
    // time, else the end of the effective time, when either reaches the second with an offset.
    let author_time = org
        .authors
        .iter()
        .filter_map(|a| a.time.as_deref())
        .filter_map(|t| map_datetime(t, ctx.report))
        .filter(|t| t.contains('T'))
        .max();
    let effective_end = r
        .get("effectiveDateTime")
        .or_else(|| r.get("effectivePeriod").and_then(|p| p.get("end")))
        .and_then(Value::as_str)
        .filter(|t| t.contains('T'))
        .map(str::to_owned);
    if let Some(issued) = author_time.or(effective_end) {
        r.insert("issued".into(), json!(issued));
        ctx.report.info(
            "results.issued.derived",
            "issued was taken from the author time or the end of the effective time",
        );
    }
    let refs: Vec<Value> = members
        .iter()
        .filter_map(|m| m["id"].as_str())
        .map(|id| json!({ "reference": ctx.reference("Observation", id) }))
        .collect();
    if !refs.is_empty() {
        r.insert("result".into(), json!(refs));
    }
    apply_performer(&mut r, org, None, ctx);

    out.push(Value::Object(r));
    out.extend(members);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn doc(organizer_attrs: &str, members: &str) -> String {
        format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section>
  <code code="30954-2" codeSystem="2.16.840.1.113883.6.1"/>
  <entry><organizer classCode="BATTERY" moodCode="EVN">
    <templateId root="2.16.840.1.113883.10.20.22.4.1"/>
    <id root="1.2.3" extension="ORG"/>
    <code code="24357-6" codeSystem="2.16.840.1.113883.6.1"/>
    {organizer_attrs}
    {members}
  </organizer></entry>
</section></component></structuredBody></component></ClinicalDocument>"#
        )
    }

    fn member(code: &str, status: &str, value: &str, time: &str) -> String {
        format!(
            r#"<component><observation classCode="OBS" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.2"/>
  <id root="1.2.3" extension="{code}"/>
  <code code="{code}" codeSystem="2.16.840.1.113883.6.1"/>
  <statusCode code="{status}"/>
  <effectiveTime value="{time}"/>
  {value}
</observation></component>"#
        )
    }

    fn run(xml: &str) -> (Vec<Value>, Report) {
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_results(&d, &mut ctx);
        (out, report)
    }

    fn kind<'a>(out: &'a [Value], t: &str) -> Vec<&'a Value> {
        out.iter().filter(|r| r["resourceType"] == t).collect()
    }

    #[test]
    fn organizer_is_a_report_that_lists_its_observations() {
        let m = member(
            "718-7",
            "completed",
            r#"<value xsi:type="PQ" value="17.2" unit="g/dL"/>"#,
            "20240101",
        ) + &member(
            "6690-2",
            "completed",
            r#"<value xsi:type="PQ" value="6.8" unit="10*3/uL"/>"#,
            "20240102",
        );
        let (out, _) = run(&doc(r#"<statusCode code="completed"/>"#, &m));
        let report = kind(&out, "DiagnosticReport")[0];
        assert_eq!(report["status"], "final");
        assert_eq!(report["result"].as_array().unwrap().len(), 2);
        assert_eq!(report["category"][0]["coding"][0]["code"], "LAB");
        // No organizer time: the span of the observations.
        assert_eq!(report["effectivePeriod"]["start"], "2024-01-01");
        assert_eq!(report["effectivePeriod"]["end"], "2024-01-02");
        assert_eq!(kind(&out, "Observation").len(), 2);
    }

    #[test]
    fn status_follows_the_ig_map() {
        for (code, want) in [
            ("completed", "final"),
            ("active", "registered"),
            ("aborted", "cancelled"),
            ("nullified", "entered-in-error"),
        ] {
            let m = member(
                "718-7",
                code,
                r#"<value xsi:type="ST">x</value>"#,
                "20240101",
            );
            let (out, _) = run(&doc(&format!(r#"<statusCode code="{code}"/>"#), &m));
            assert_eq!(kind(&out, "Observation")[0]["status"], want, "{code}");
        }
    }

    #[test]
    fn value_types_map_to_value_elements() {
        let cases = [
            (r#"<value xsi:type="ST">Yellow</value>"#, "valueString"),
            (r#"<value xsi:type="INT" value="3"/>"#, "valueInteger"),
            (r#"<value xsi:type="REAL" value="2.50"/>"#, "valueQuantity"),
            (
                r#"<value xsi:type="CD" code="1" codeSystem="2.16.840.1.113883.6.96"/>"#,
                "valueCodeableConcept",
            ),
            (r#"<value xsi:type="BL" value="true"/>"#, "valueBoolean"),
            (
                r#"<value xsi:type="IVL_PQ"><low value="1" unit="mg"/><high value="2" unit="mg"/></value>"#,
                "valueRange",
            ),
            (
                r#"<value xsi:type="PQ" nullFlavor="NI"/>"#,
                "dataAbsentReason",
            ),
        ];
        for (value, key) in cases {
            let (out, _) = run(&doc("", &member("1-1", "completed", value, "20240101")));
            assert!(kind(&out, "Observation")[0].get(key).is_some(), "{key}");
        }
    }

    #[test]
    fn only_normal_reference_ranges_are_kept() {
        let value = r#"<value xsi:type="PQ" value="5" unit="mg"/>
<referenceRange><observationRange><value xsi:type="IVL_PQ"><low value="1" unit="mg"/><high value="9" unit="mg"/></value></observationRange></referenceRange>
<referenceRange><observationRange><interpretationCode code="H" codeSystem="2.16.840.1.113883.5.83"/><value xsi:type="IVL_PQ"><low value="9" unit="mg"/></value></observationRange></referenceRange>"#;
        let (out, report) = run(&doc("", &member("1-1", "completed", value, "20240101")));
        let ranges = kind(&out, "Observation")[0]["referenceRange"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0]["high"]["value"], 9);
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "observation.reference-range.not-normal"));
    }

    #[test]
    fn decimals_keep_their_written_precision() {
        let (out, _) = run(&doc(
            "",
            &member(
                "1-1",
                "completed",
                r#"<value xsi:type="PQ" value="5.40" unit="mg"/>"#,
                "20240101",
            ),
        ));
        assert!(serde_json::to_string(&out).unwrap().contains("5.40"));
    }
}
