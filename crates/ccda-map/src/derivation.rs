//! One derivation `Provenance` per converted document.
//!
//! It says: these resources were derived from that document (the `Composition`), by this
//! converter. The assembler agent is a `Device` naming the converter and its version. Author
//! `Provenance` (see `author.rs`) says who wrote a fact; this one says where the fact came from.
//! See ADR 0045.

use serde_json::{json, Value};

use crate::{map_datetime, Context};

const PARTICIPANT_TYPE: &str = "http://terminology.hl7.org/CodeSystem/provenance-participant-type";
const ACTIVITY_SYSTEM: &str = "http://hl7.org/fhir/w3c-provenance-activity-type";
/// The R4 value set for `Provenance.activity` has no derivation code, so the activity also
/// carries `create` from it: the converter creates the FHIR resources.
const DATA_OPERATION: &str = "http://terminology.hl7.org/CodeSystem/v3-DataOperation";
const ENTITY_ROLE: &str = "source";

/// Add the converter `Device` and the derivation `Provenance` to the extra resources.
///
/// `recorded` is the document's own time, so the output stays deterministic. A document time
/// without an offset cannot be an `instant`, so then nothing is added and the report says so.
pub fn map_derivation(
    document_time: Option<&str>,
    composition: &Value,
    resources: &[Value],
    ctx: &mut Context,
) {
    let recorded = document_time
        .and_then(|t| map_datetime(t, ctx.report))
        .filter(|d| d.contains('T'));
    let Some(recorded) = recorded else {
        ctx.report.warn(
            "provenance.derivation.unavailable",
            "the document time has no offset, so no derivation Provenance was created (recorded is required)",
        );
        return;
    };
    let Some(composition_id) = composition.get("id").and_then(Value::as_str) else {
        return;
    };

    let device_id = ctx.next_id("device");
    ctx.push_extra(json!({
        "resourceType": "Device",
        "id": device_id,
        "status": "active",
        "deviceName": [{ "name": "ccda2fhir", "type": "user-friendly-name" }],
        "version": [{ "value": env!("CARGO_PKG_VERSION") }],
    }));

    let mut targets = vec![json!({ "reference": ctx.patient })];
    for r in resources {
        if let (Some(t), Some(id)) = (
            r.get("resourceType").and_then(Value::as_str),
            r.get("id").and_then(Value::as_str),
        ) {
            targets.push(json!({ "reference": ctx.reference(t, id) }));
        }
    }

    let id = ctx.next_id("provenance");
    ctx.push_extra(json!({
        "resourceType": "Provenance",
        "id": id,
        "target": targets,
        "recorded": recorded,
        "activity": { "coding": [
            { "system": ACTIVITY_SYSTEM, "code": "Derivation" },
            { "system": DATA_OPERATION, "code": "CREATE" },
        ] },
        "agent": [{
            "type": { "coding": [{ "system": PARTICIPANT_TYPE, "code": "assembler" }] },
            "who": { "reference": ctx.reference("Device", &device_id) },
        }],
        "entity": [{
            "role": ENTITY_ROLE,
            "what": { "reference": ctx.reference("Composition", composition_id) },
        }],
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(time: Option<&str>) -> (Vec<Value>, Report) {
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let composition = json!({ "resourceType": "Composition", "id": "composition-1" });
        let resources = [json!({ "resourceType": "Condition", "id": "condition-1" })];
        map_derivation(time, &composition, &resources, &mut ctx);
        let extra = ctx.take_extra();
        (extra, report)
    }

    #[test]
    fn provenance_points_from_resources_to_the_composition() {
        let (extra, _) = run(Some("20250102120000-0500"));
        let p = extra
            .iter()
            .find(|r| r["resourceType"] == "Provenance")
            .unwrap();
        assert_eq!(p["recorded"], "2025-01-02T12:00:00-05:00");
        assert_eq!(p["target"].as_array().unwrap().len(), 2);
        assert_eq!(p["agent"][0]["type"]["coding"][0]["code"], "assembler");
        assert!(p["entity"][0]["what"]["reference"]
            .as_str()
            .unwrap()
            .starts_with("urn:uuid:"));
        assert!(extra.iter().any(|r| r["resourceType"] == "Device"));
    }

    #[test]
    fn time_without_offset_gives_nothing_and_is_reported() {
        let (extra, report) = run(Some("20250102"));
        assert!(extra.is_empty());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "provenance.derivation.unavailable"));
    }
}
