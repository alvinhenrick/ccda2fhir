//! A planned encounter with mood `APT` (an appointment) to FHIR `Appointment`.
//!
//! Other planned encounters stay `ServiceRequest`s (see `service_request.rs`).

use ccda_model::Statement;
use serde_json::{json, Map, Value};

use crate::author::register_author_agent;
use crate::{map_codeable_concept, map_datetime, map_identifier, Context};

fn status(s: &Statement, ctx: &mut Context) -> &'static str {
    match s.status_code.as_deref() {
        Some("active" | "new") => "booked",
        Some("completed") => "fulfilled",
        Some("aborted" | "cancelled") => "cancelled",
        Some("held" | "suspended") => "pending",
        Some("nullified") => "entered-in-error",
        other => {
            ctx.report.info(
                "appointment.status.assumed",
                if other.is_some() {
                    "a statusCode is not a known appointment status; status was set to booked"
                } else {
                    "an appointment has no statusCode; status was set to booked"
                },
            );
            "booked"
        }
    }
}

/// Map one appointment.
pub(crate) fn map_appointment(s: &Statement, ctx: &mut Context) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Appointment"));
    r.insert("id".into(), json!(ctx.next_id("appointment")));
    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    let st = status(s, ctx);
    r.insert("status".into(), json!(st));
    if let Some(code) = s
        .code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
    {
        r.insert("serviceType".into(), json!([code]));
    }
    if let Some(text) = s.text.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        r.insert("description".into(), json!(text));
    }

    if let Some(t) = &s.effective_time {
        let low = t
            .value
            .as_deref()
            .or(t.low.as_ref().and_then(|p| p.value.as_deref()))
            .and_then(|v| map_datetime(v, ctx.report));
        let high = t
            .value
            .as_deref()
            .or(t.high.as_ref().and_then(|p| p.value.as_deref()))
            .and_then(|v| map_datetime(v, ctx.report));
        // `start` and `end` are instants (time and offset); a date alone is a requested period.
        let is_instant = |d: &Option<String>| d.as_deref().is_some_and(|d| d.contains('T'));
        if is_instant(&low) {
            r.insert("start".into(), json!(low));
            if is_instant(&high) {
                r.insert("end".into(), json!(high));
            }
        } else {
            let mut period = Map::new();
            if let Some(l) = low {
                period.insert("start".into(), json!(l));
            }
            if let Some(h) = high {
                period.insert("end".into(), json!(h));
            }
            if !period.is_empty() {
                r.insert("requestedPeriod".into(), json!([period]));
            }
        }
    }

    let mut participants =
        vec![json!({ "actor": { "reference": ctx.patient }, "status": "accepted" })];
    for p in s.performers.iter().chain(s.authors.iter()) {
        if let Some(reference) = register_author_agent(p, ctx) {
            participants.push(json!({ "actor": { "reference": reference }, "status": "accepted" }));
        }
    }
    r.insert("participant".into(), json!(participants));
    Value::Object(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(effective: &str) -> Value {
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><component><structuredBody><component><section><entry>
<encounter classCode="ENC" moodCode="APT"><templateId root="2.16.840.1.113883.10.20.22.4.40"/>
<id root="1.2.3" extension="A1"/><code code="99213" codeSystem="2.16.840.1.113883.6.12"/>
<statusCode code="active"/>{effective}</encounter></entry></section></component></structuredBody></component></ClinicalDocument>"#
        );
        let doc = ccda_model::parse(xml.as_bytes()).unwrap();
        let s = &doc.sections[0].entries[0].statement;
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        map_appointment(s, &mut ctx)
    }

    #[test]
    fn instant_becomes_start() {
        let a = run(r#"<effectiveTime value="20250301101500-0500"/>"#);
        assert_eq!(a["status"], "booked");
        assert_eq!(a["start"], "2025-03-01T10:15:00-05:00");
        assert_eq!(a["participant"][0]["status"], "accepted");
    }

    #[test]
    fn date_only_becomes_requested_period() {
        let a = run(r#"<effectiveTime value="20250301"/>"#);
        assert!(a.get("start").is_none());
        assert_eq!(a["requestedPeriod"][0]["start"], "2025-03-01");
    }
}
