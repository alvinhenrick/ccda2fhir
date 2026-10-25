//! Procedure Activity Procedure / Act (`procedure`, `act`) to FHIR `Procedure`.
//!
//! Rules come from the IG page "CF-procedures", its CSV and the ConceptMap `CF-ProcedureStatus`.
//! Procedure Activity Observation is an observation, not a procedure, and is reported. A planned
//! procedure (mood INT) is a `ServiceRequest` in FHIR and is reported until that mapper exists.

use ccda_model::{ClinicalDocument, Statement, StatementKind, Value as Cv};
use serde_json::{json, Map, Value};

use crate::author::{apply_authors, register_organization, register_practitioner, AuthorFields};
use crate::util::nested;
use crate::{
    data_absent_reason_extension, map_codeable_concept, map_datetime, map_identifier, Context,
};

const PROCEDURE_ACTIVITY_PROCEDURE: &str = "2.16.840.1.113883.10.20.22.4.14";
const PROCEDURE_ACTIVITY_ACT: &str = "2.16.840.1.113883.10.20.22.4.12";
const PROCEDURE_ACTIVITY_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.13";
const INDICATION: &str = "2.16.840.1.113883.10.20.22.4.19";

/// `Procedure.recorder` holds the latest author; there is no recorded date element.
const FIELDS: AuthorFields = AuthorFields {
    recorder: Some("recorder"),
    date: "_authored",
};

/// Map every Procedure Activity (Procedure or Act) in the document.
pub fn map_procedures(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.has_template(PROCEDURE_ACTIVITY_OBSERVATION) {
                ctx.report.info(
                    "procedure.observation.unsupported",
                    "a Procedure Activity Observation is an observation and was not mapped as a Procedure",
                );
                continue;
            }
            let is_procedure = s.kind == StatementKind::Procedure
                && s.has_template(PROCEDURE_ACTIVITY_PROCEDURE)
                || s.kind == StatementKind::Act && s.has_template(PROCEDURE_ACTIVITY_ACT);
            if !is_procedure {
                continue;
            }
            match s.mood_code.as_deref() {
                Some("EVN") | None => out.push(map_activity(s, ctx)),
                Some(_) => ctx.report.info(
                    "procedure.intent.unsupported",
                    "a planned or requested procedure (not an event) was not mapped; FHIR uses ServiceRequest",
                ),
            }
        }
    }
    out
}

/// `CF-ProcedureStatus` (ActStatus to event-status), extended for the codes the map omits.
fn status_of(code: &str) -> Option<&'static str> {
    Some(match code {
        "completed" => "completed",
        "active" => "in-progress",
        "aborted" => "stopped",
        "cancelled" => "not-done",
        "nullified" => "entered-in-error",
        "held" | "suspended" => "on-hold",
        "new" => "preparation",
        _ => return None,
    })
}

fn dar(nf: &str, ctx: &mut Context) -> Value {
    json!({ "extension": [data_absent_reason_extension(nf, ctx.report)] })
}

fn map_activity(s: &Statement, ctx: &mut Context) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Procedure"));
    r.insert("id".into(), json!(ctx.next_id("procedure")));

    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }

    let status = if s.negation_ind {
        "not-done"
    } else {
        match s.status_code.as_deref() {
            Some(code) => status_of(code).unwrap_or_else(|| {
                ctx.report.warn(
                    "procedure.status.unmapped",
                    "a statusCode is not in the IG's status map; status was set to unknown",
                );
                "unknown"
            }),
            None => {
                ctx.report.warn(
                    "procedure.status.missing",
                    "a procedure has no statusCode; status was set to unknown",
                );
                "unknown"
            }
        }
    };
    r.insert("status".into(), json!(status));

    let code = s.code.as_ref().and_then(|c| {
        map_codeable_concept(c, ctx.terminology, ctx.report)
            .or_else(|| c.null_flavor.as_deref().map(|nf| dar(nf, ctx)))
    });
    r.insert(
        "code".into(),
        code.unwrap_or_else(|| {
            ctx.report.warn(
                "procedure.code.missing",
                "a procedure has no code; code was marked unknown",
            );
            dar("UNK", ctx)
        }),
    );
    r.insert("subject".into(), json!({ "reference": ctx.patient }));

    performed(&mut r, s, ctx);

    if let Some(site) = s
        .target_site_codes
        .first()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
    {
        r.insert("bodySite".into(), json!([site]));
    }
    if !s.method_codes.is_empty() || s.approach_site_code.is_some() {
        ctx.report.info(
            "procedure.method.unsupported",
            "a method or approach on a procedure has no FHIR element (IG: not supported); it was omitted",
        );
    }

    let mut performers: Vec<Value> = Vec::new();
    for performer in &s.performers {
        let actor = register_practitioner(performer, ctx).or_else(|| {
            performer
                .organization
                .as_ref()
                .and_then(|org| register_organization(org, ctx))
        });
        if let Some(reference) = actor {
            let entry = json!({ "actor": { "reference": reference } });
            if !performers.contains(&entry) {
                performers.push(entry);
            }
        }
    }
    if !performers.is_empty() {
        r.insert("performer".into(), json!(performers));
    }

    if s.participants
        .iter()
        .any(|p| p.type_code.as_deref() == Some("LOC"))
    {
        ctx.report.info(
            "procedure.location.unsupported",
            "a service delivery location was not mapped yet (needs a Location resource)",
        );
    }

    let mut reasons: Vec<Value> = Vec::new();
    for related in nested(s) {
        if related.has_template(INDICATION) {
            if let Some(Cv::Coded(code)) = related.values.first() {
                if let Some(c) = map_codeable_concept(code, ctx.terminology, ctx.report) {
                    reasons.push(c);
                }
            }
        }
    }
    if !reasons.is_empty() {
        r.insert("reasonCode".into(), json!(reasons));
    }

    let authors: Vec<_> = s.authors.iter().collect();
    apply_authors(&mut r, FIELDS, &authors, ctx);
    r.remove("_authored");
    Value::Object(r)
}

/// `performedDateTime` from the point value, or `performedPeriod` from the interval; a data
/// absent reason when there is no time at all (the IG's rule).
fn performed(r: &mut Map<String, Value>, s: &Statement, ctx: &mut Context) {
    let time = s.effective_time.as_ref();
    if let Some(raw) = time.and_then(|t| t.value.as_deref()) {
        if let Some(d) = map_datetime(raw, ctx.report) {
            r.insert("performedDateTime".into(), json!(d));
            return;
        }
        r.insert("_performedDateTime".into(), dar("UNK", ctx));
        return;
    }
    let mut period = Map::new();
    for (key, point) in [
        ("start", time.and_then(|t| t.low.as_ref())),
        ("end", time.and_then(|t| t.high.as_ref())),
    ] {
        if let Some(d) = point
            .and_then(|p| p.value.as_deref())
            .and_then(|raw| map_datetime(raw, ctx.report))
        {
            period.insert(key.into(), json!(d));
        }
    }
    if !period.is_empty() {
        r.insert("performedPeriod".into(), Value::Object(period));
        return;
    }
    let nf = time
        .and_then(|t| {
            t.null_flavor
                .clone()
                .or_else(|| t.low.as_ref().and_then(|l| l.null_flavor.clone()))
        })
        .unwrap_or_else(|| {
            ctx.report.warn(
                "procedure.performed.missing",
                "a procedure has no date; performed was marked unknown",
            );
            "UNK".to_owned()
        });
    r.insert("_performedDateTime".into(), dar(&nf, ctx));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn doc(kind: &str, tmpl: &str, attrs: &str, body: &str) -> String {
        format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section>
  <code code="47519-4" codeSystem="2.16.840.1.113883.6.1"/>
  <entry><{kind} classCode="PROC" moodCode="{}" {attrs}>
    <templateId root="{tmpl}"/>
    <id root="1.2.3" extension="P-1"/>
    <code code="80146002" codeSystem="2.16.840.1.113883.6.96"/>
    {body}
  </{kind}></entry>
</section></component></structuredBody></component></ClinicalDocument>"#,
            if attrs.contains("INT") { "INT" } else { "EVN" }
        )
    }

    fn run(xml: &str) -> (Vec<Value>, Report) {
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_procedures(&d, &mut ctx);
        (out, report)
    }

    const PROC: &str = "2.16.840.1.113883.10.20.22.4.14";

    #[test]
    fn status_map_and_negation() {
        for (code, want) in [
            ("completed", "completed"),
            ("active", "in-progress"),
            ("aborted", "stopped"),
            ("cancelled", "not-done"),
        ] {
            let (out, _) = run(&doc(
                "procedure",
                PROC,
                "",
                &format!(r#"<statusCode code="{code}"/>"#),
            ));
            assert_eq!(out[0]["status"], want, "{code}");
        }
        let (out, _) = run(&doc(
            "procedure",
            PROC,
            r#"negationInd="true""#,
            r#"<statusCode code="completed"/>"#,
        ));
        assert_eq!(out[0]["status"], "not-done");
    }

    #[test]
    fn point_period_and_missing_times() {
        let (out, _) = run(&doc(
            "procedure",
            PROC,
            "",
            r#"<statusCode code="completed"/><effectiveTime value="20240102"/>"#,
        ));
        assert_eq!(out[0]["performedDateTime"], "2024-01-02");
        let (out, _) = run(&doc(
            "procedure",
            PROC,
            "",
            r#"<statusCode code="completed"/><effectiveTime><low value="20240102"/><high value="20240103"/></effectiveTime>"#,
        ));
        assert_eq!(out[0]["performedPeriod"]["end"], "2024-01-03");
        let (out, report) = run(&doc(
            "procedure",
            PROC,
            "",
            r#"<statusCode code="completed"/>"#,
        ));
        assert_eq!(
            out[0]["_performedDateTime"]["extension"][0]["valueCode"],
            "unknown"
        );
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "procedure.performed.missing"));
    }

    #[test]
    fn planned_procedures_and_observations_are_reported_not_mapped() {
        let (out, report) = run(&doc("procedure", PROC, r#"x="INT""#, ""));
        assert!(out.is_empty());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "procedure.intent.unsupported"));
        let (out, report) = run(&doc(
            "observation",
            "2.16.840.1.113883.10.20.22.4.13",
            "",
            "",
        ));
        assert!(out.is_empty());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "procedure.observation.unsupported"));
    }

    #[test]
    fn the_act_variant_maps_the_same_way() {
        let (out, _) = run(&doc(
            "act",
            "2.16.840.1.113883.10.20.22.4.12",
            "",
            r#"<statusCode code="completed"/><effectiveTime value="2024"/>"#,
        ));
        assert_eq!(out[0]["resourceType"], "Procedure");
        assert_eq!(out[0]["performedDateTime"], "2024");
    }
}
