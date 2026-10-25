//! Vital Signs Organizer and Vital Sign Observations to FHIR `Observation`s.
//!
//! Rules come from the IG page "CF-vitals": the organizer becomes a panel Observation (LOINC
//! 85353-1) with `hasMember` links; blood pressure (8480-6 and 8462-4) and pulse oximetry
//! (59408-5/2708-6 with 3150-0/3151-8) are combined into one Observation with components, as US
//! Core requires.

use ccda_model::{ClinicalDocument, Statement, StatementKind};
use serde_json::{json, Map, Value};

use crate::observation::{apply_performer, base, category, insert_details, insert_value, status};
use crate::util::LOINC;
use crate::{map_codeable_concept, Context};

const VITALS_ORGANIZER: &str = "2.16.840.1.113883.10.20.22.4.26";
const VITAL_SIGN: &str = "2.16.840.1.113883.10.20.22.4.27";

const LOINC_URI: &str = "http://loinc.org";
const SYSTOLIC: &str = "8480-6";
const DIASTOLIC: &str = "8462-4";
const SAT_PULSE_OX: &str = "59408-5";
const SAT_ARTERIAL: &str = "2708-6";
const O2_CONCENTRATION: &str = "3150-0";
const O2_FLOW: &str = "3151-8";

fn loinc_code(s: &Statement) -> Option<&str> {
    s.code
        .as_ref()
        .filter(|c| c.code_system.as_deref() == Some(LOINC))
        .and_then(|c| c.code.as_deref())
}

fn vital_category() -> Value {
    category("vital-signs", "Vital Signs")
}

/// Map every Vital Signs Organizer (and any loose Vital Sign Observation) in the document.
pub fn map_vitals(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind == StatementKind::Organizer && s.has_template(VITALS_ORGANIZER) {
                let members: Vec<&Statement> = s
                    .related
                    .iter()
                    .map(|r| &r.statement)
                    .filter(|m| m.kind == StatementKind::Observation && m.has_template(VITAL_SIGN))
                    .collect();
                map_group(&members, Some(s), ctx, &mut out);
            } else if s.kind == StatementKind::Observation && s.has_template(VITAL_SIGN) {
                map_group(&[s], None, ctx, &mut out);
            }
        }
    }
    out
}

fn map_group(
    members: &[&Statement],
    organizer: Option<&Statement>,
    ctx: &mut Context,
    out: &mut Vec<Value>,
) {
    let find = |code: &str| members.iter().position(|m| loinc_code(m) == Some(code));
    let mut used = vec![false; members.len()];
    let mut produced: Vec<Value> = Vec::new();

    // Blood pressure needs both readings.
    match (find(SYSTOLIC), find(DIASTOLIC)) {
        (Some(sys), Some(dia)) => {
            used[sys] = true;
            used[dia] = true;
            produced.push(combined(
                members[sys],
                &[members[sys], members[dia]],
                blood_pressure_code(),
                organizer,
                ctx,
            ));
        }
        (Some(_), None) | (None, Some(_)) => ctx.report.info(
            "vitals.blood-pressure.incomplete",
            "only one of systolic and diastolic pressure is present, so they were mapped separately",
        ),
        (None, None) => {}
    }

    // Pulse oximetry: the saturation is the value; oxygen concentration and flow are components.
    if let Some(sat) = find(SAT_PULSE_OX).or_else(|| find(SAT_ARTERIAL)) {
        used[sat] = true;
        let mut parts = Vec::new();
        for code in [O2_CONCENTRATION, O2_FLOW] {
            if let Some(i) = find(code) {
                used[i] = true;
                parts.push(members[i]);
            }
        }
        let mut r = single(members[sat], organizer, ctx);
        r.insert("code".into(), pulse_oximetry_code());
        if !parts.is_empty() {
            let components: Vec<Value> = parts.iter().map(|p| component(p, ctx)).collect();
            r.insert("component".into(), json!(components));
        }
        produced.push(Value::Object(r));
    }

    for (i, m) in members.iter().enumerate() {
        if !used[i] {
            produced.push(Value::Object(single(m, organizer, ctx)));
        }
    }

    if let Some(org) = organizer {
        out.push(panel(org, &produced, ctx));
    }
    out.extend(produced);
}

/// One vital sign as its own Observation.
fn single(s: &Statement, organizer: Option<&Statement>, ctx: &mut Context) -> Map<String, Value> {
    let mut r = base(s, "vitalsign", ctx);
    r.insert("category".into(), vital_category());
    apply_performer(&mut r, s, organizer, ctx);
    r
}

/// An Observation with components, no value of its own. Identifiers of all parts are kept; the
/// first part supplies status, time and the performer.
fn combined(
    primary: &Statement,
    parts: &[&Statement],
    code: Value,
    organizer: Option<&Statement>,
    ctx: &mut Context,
) -> Value {
    let mut shell = primary.clone();
    shell.values.clear();
    shell.interpretation_codes.clear();
    shell.reference_ranges.clear();
    shell.method_codes.clear();
    shell.target_site_codes.clear();
    let mut r = base(&shell, "vitalsign", ctx);
    r.insert("code".into(), code);
    r.insert("category".into(), vital_category());

    let mut identifiers: Vec<Value> = Vec::new();
    for part in parts {
        for id in &part.ids {
            if let Some(v) = crate::map_identifier(id, ctx.terminology, ctx.report) {
                if !identifiers.contains(&v) {
                    identifiers.push(v);
                }
            }
        }
    }
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    let components: Vec<Value> = parts.iter().map(|p| component(p, ctx)).collect();
    r.insert("component".into(), json!(components));
    apply_performer(&mut r, primary, organizer, ctx);
    Value::Object(r)
}

fn component(s: &Statement, ctx: &mut Context) -> Value {
    let mut c = Map::new();
    if let Some(code) = s
        .code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
    {
        c.insert("code".into(), code);
    }
    insert_value(&mut c, s, ctx);
    insert_details(&mut c, s, ctx);
    // `method` and `bodySite` have no place on a component.
    for key in ["method", "bodySite"] {
        if c.remove(key).is_some() {
            ctx.report.info(
                "vitals.component.detail-dropped",
                "a method or site on a component part has no FHIR element and was omitted",
            );
        }
    }
    Value::Object(c)
}

/// The panel Observation for a Vital Signs Organizer.
fn panel(org: &Statement, members: &[Value], ctx: &mut Context) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Observation"));
    r.insert("id".into(), json!(ctx.next_id("vitalsigns-panel")));
    let identifiers: Vec<Value> = org
        .ids
        .iter()
        .filter_map(|id| crate::map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    r.insert("status".into(), json!(status(org, ctx)));
    r.insert("category".into(), vital_category());
    r.insert(
        "code".into(),
        json!({
            "coding": [{
                "system": LOINC_URI,
                "code": "85353-1",
                "display": "Vital signs, weight, height, head circumference, oxygen saturation and BMI panel"
            }],
            "text": "Vital Signs Panel"
        }),
    );
    r.insert("subject".into(), json!({ "reference": ctx.patient }));

    // The organizer's time; if missing, the span of its members' times.
    crate::observation::insert_effective(&mut r, org, ctx);
    if !r.contains_key("effectiveDateTime") && !r.contains_key("effectivePeriod") {
        let mut times: Vec<&str> = members
            .iter()
            .filter_map(|m| {
                m["effectiveDateTime"]
                    .as_str()
                    .or_else(|| m["effectivePeriod"]["start"].as_str())
            })
            .collect();
        times.sort_unstable();
        match (times.first(), times.last()) {
            (Some(a), Some(b)) if a == b => {
                r.insert("effectiveDateTime".into(), json!(a));
            }
            (Some(a), Some(b)) => {
                r.insert("effectivePeriod".into(), json!({ "start": a, "end": b }));
            }
            _ => {}
        }
    }

    let refs: Vec<Value> = members
        .iter()
        .filter_map(|m| m["id"].as_str())
        .map(|id| json!({ "reference": ctx.reference("Observation", id) }))
        .collect();
    if !refs.is_empty() {
        r.insert("hasMember".into(), json!(refs));
    }
    apply_performer(&mut r, org, None, ctx);
    Value::Object(r)
}

fn blood_pressure_code() -> Value {
    json!({
        "coding": [{ "system": LOINC_URI, "code": "85354-9", "display": "Blood pressure panel with all children optional" }],
        "text": "Blood pressure"
    })
}

fn pulse_oximetry_code() -> Value {
    json!({
        "coding": [
            { "system": LOINC_URI, "code": SAT_PULSE_OX, "display": "Oxygen saturation in Arterial blood by Pulse oximetry" },
            { "system": LOINC_URI, "code": SAT_ARTERIAL, "display": "Oxygen saturation in Arterial blood" }
        ],
        "text": "Oxygen saturation"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn vital(code: &str, value: &str, unit: &str) -> String {
        format!(
            r#"<component><observation classCode="OBS" moodCode="EVN">
  <templateId root="2.16.840.1.113883.10.20.22.4.27"/>
  <id root="1.2.3" extension="V-{code}"/>
  <code code="{code}" codeSystem="2.16.840.1.113883.6.1"/>
  <statusCode code="completed"/>
  <effectiveTime value="20240101"/>
  <value xsi:type="PQ" value="{value}" unit="{unit}"/>
</observation></component>"#
        )
    }

    fn organizer(body: &str) -> String {
        format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<component><structuredBody><component><section>
  <code code="8716-3" codeSystem="2.16.840.1.113883.6.1"/>
  <entry><organizer classCode="CLUSTER" moodCode="EVN">
    <templateId root="2.16.840.1.113883.10.20.22.4.26"/>
    <id root="1.2.3" extension="ORG"/>
    <statusCode code="completed"/>
    {body}
  </organizer></entry>
</section></component></structuredBody></component></ClinicalDocument>"#
        )
    }

    fn run(xml: &str) -> (Vec<Value>, Report) {
        let doc = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let out = map_vitals(&doc, &mut ctx);
        (out, report)
    }

    fn by_code<'a>(out: &'a [Value], code: &str) -> &'a Value {
        out.iter()
            .find(|r| r["code"]["coding"][0]["code"] == code)
            .unwrap()
    }

    #[test]
    fn blood_pressure_is_one_observation_with_two_components() {
        let body = vital("8480-6", "120", "mm[Hg]") + &vital("8462-4", "80", "mm[Hg]");
        let (out, _) = run(&organizer(&body));
        assert_eq!(out.len(), 2, "panel plus one blood pressure");
        let bp = by_code(&out, "85354-9");
        assert!(bp.get("valueQuantity").is_none());
        let comps = bp["component"].as_array().unwrap();
        assert_eq!(comps.len(), 2);
        assert_eq!(comps[0]["code"]["coding"][0]["code"], "8480-6");
        assert_eq!(comps[1]["valueQuantity"]["value"], 80);
        assert_eq!(bp["identifier"].as_array().unwrap().len(), 2);
        assert_eq!(bp["category"][0]["coding"][0]["code"], "vital-signs");
    }

    #[test]
    fn a_lone_pressure_stays_separate_and_is_reported() {
        let (out, report) = run(&organizer(&vital("8480-6", "120", "mm[Hg]")));
        assert!(by_code(&out, "8480-6").get("component").is_none());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "vitals.blood-pressure.incomplete"));
    }

    #[test]
    fn pulse_oximetry_carries_flow_and_concentration_as_components() {
        let body = vital("59408-5", "97", "%")
            + &vital("3151-8", "2", "L/min")
            + &vital("3150-0", "30", "%");
        let (out, _) = run(&organizer(&body));
        let ox = by_code(&out, "59408-5");
        assert_eq!(ox["code"]["coding"][1]["code"], "2708-6");
        assert_eq!(ox["valueQuantity"]["value"], 97);
        assert_eq!(ox["component"].as_array().unwrap().len(), 2);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn panel_links_every_member_and_takes_the_span_of_their_times() {
        let body = vital("8867-4", "70", "/min") + &vital("9279-1", "16", "/min");
        let (out, _) = run(&organizer(&body));
        let panel = by_code(&out, "85353-1");
        assert_eq!(panel["hasMember"].as_array().unwrap().len(), 2);
        assert_eq!(panel["effectiveDateTime"], "2024-01-01");
        assert_eq!(panel["status"], "final");
    }

    #[test]
    fn non_ucum_units_are_replaced_in_code_and_kept_in_unit() {
        let (out, report) = run(&organizer(&vital("29463-7", "154.5", "lb")));
        let w = &by_code(&out, "29463-7")["valueQuantity"];
        assert_eq!(w["code"], "[lb_av]");
        assert_eq!(w["unit"], "lb");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "quantity.unit.normalized"));
    }

    #[test]
    fn organizer_without_members_still_gets_a_panel_without_members() {
        let (out, _) = run(&organizer(""));
        assert_eq!(out.len(), 1);
        assert!(out[0].get("hasMember").is_none());
    }
}
