//! Product Instance (UDI) participants to FHIR `Device`.
//!
//! A Product Instance (template 4.37) is a `participant` (typeCode DEV or PRD) of a procedure,
//! supply or organizer, usually in the Medical Equipment section. Its `id` under the FDA UDI root
//! carries the UDI in human readable form; `playingDevice/code` is the device type. The result
//! follows the US Core Implantable Device profile. The same UDI seen twice is one Device.
//! (No IG page covers devices; this follows C-CDA and the US Core profile.)

use ccda_model::{ClinicalDocument, Participant, Statement};
use serde_json::{json, Map, Value};

use crate::{map_codeable_concept, map_datetime, map_identifier, Context};

const PRODUCT_INSTANCE: &str = "2.16.840.1.113883.10.20.22.4.37";
const NON_MEDICINAL_SUPPLY: &str = "2.16.840.1.113883.10.20.22.4.50";
/// The FDA UDI root used in `participantRole/id`.
const UDI_ROOT: &str = "2.16.840.1.113883.3.3719";

/// Map every Product Instance in the document, one Device per distinct UDI. A Non-Medicinal
/// Supply Activity (4.50) also gets a `DeviceUseStatement`: the patient uses this device.
pub fn map_devices(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut found: Vec<(&Statement, &Participant)> = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            collect(&entry.statement, &mut found);
        }
    }
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (owner, p) in found {
        let hrf = udi_of(p);
        let known = hrf
            .as_ref()
            .filter(|h| seen.contains(h))
            .and_then(|h| ctx.devices.get(h))
            .cloned();
        let device_id = if let Some(id) = known {
            id
        } else {
            let id = match &hrf {
                Some(h) => device_id_for(h, ctx),
                None => ctx.next_id("device"),
            };
            let device = map_device(
                p,
                hrf.clone(),
                !owner.has_template(NON_MEDICINAL_SUPPLY),
                id.clone(),
                ctx,
            );
            if let Some(h) = hrf {
                seen.push(h);
            }
            out.push(device);
            id
        };
        if owner.has_template(NON_MEDICINAL_SUPPLY) {
            out.push(device_use_statement(owner, &device_id, ctx));
        }
    }
    out
}

fn collect<'a>(s: &'a Statement, out: &mut Vec<(&'a Statement, &'a Participant)>) {
    out.extend(
        s.participants
            .iter()
            .filter(|p| p.template_ids.iter().any(|t| t.root == PRODUCT_INSTANCE))
            .map(|p| (s, p)),
    );
    for r in &s.related {
        collect(&r.statement, out);
    }
}

/// `DeviceUseStatement` for a Non-Medicinal Supply Activity (a cane, eyeglasses, a wheelchair).
fn device_use_statement(s: &Statement, device_id: &str, ctx: &mut Context) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("DeviceUseStatement"));
    r.insert("id".into(), json!(ctx.next_id("deviceusestatement")));
    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|i| map_identifier(i, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    let status = match s.status_code.as_deref() {
        Some("active") => "active",
        Some("completed") => "completed",
        Some("aborted") => "stopped",
        Some("suspended") => "on-hold",
        Some("nullified") => "entered-in-error",
        _ => {
            ctx.report.info(
                "devicestatement.status.assumed",
                "a supply has no usable statusCode; status was set to active",
            );
            "active"
        }
    };
    r.insert("status".into(), json!(status));
    r.insert("subject".into(), json!({ "reference": ctx.patient }));
    r.insert(
        "device".into(),
        json!({ "reference": ctx.reference("Device", device_id) }),
    );
    if let Some(t) = &s.effective_time {
        let mut period = Map::new();
        let start = t.low.as_ref().and_then(|l| l.value.as_deref());
        let end = t.high.as_ref().and_then(|h| h.value.as_deref());
        for (key, raw) in [("start", start), ("end", end)] {
            if let Some(d) = raw.and_then(|raw| map_datetime(raw, ctx.report)) {
                period.insert(key.into(), json!(d));
            }
        }
        if !period.is_empty() {
            r.insert("timingPeriod".into(), Value::Object(period));
        }
    }
    Value::Object(r)
}

/// The id of the Device for a UDI, allocated on first use so that a procedure can reference it
/// before the devices are mapped.
fn device_id_for(hrf: &str, ctx: &mut Context) -> String {
    if let Some(id) = ctx.devices.get(hrf) {
        return id.clone();
    }
    let id = ctx.next_id("device");
    ctx.devices.insert(hrf.to_owned(), id.clone());
    id
}

/// References to the Devices whose Product Instance is a participant of this statement.
pub(crate) fn device_references(s: &Statement, ctx: &mut Context) -> Vec<String> {
    let mut refs: Vec<String> = Vec::new();
    for p in s
        .participants
        .iter()
        .filter(|p| p.template_ids.iter().any(|t| t.root == PRODUCT_INSTANCE))
    {
        if let Some(hrf) = udi_of(p) {
            let id = device_id_for(&hrf, ctx);
            let reference = ctx.reference("Device", &id);
            if !refs.contains(&reference) {
                refs.push(reference);
            }
        }
    }
    refs
}

/// The fields of a GS1 UDI in human readable form, e.g. `(01)0064...(17)180421(10)LOT(21)SER`.
#[derive(Default, Debug, PartialEq, Eq)]
struct Gs1 {
    manufacture: Option<String>,
    expiration: Option<String>,
    lot: Option<String>,
    serial: Option<String>,
}

/// GS1 dates are `YYMMDD`; a day of `00` means "the month" and gives a partial date.
fn gs1_date(v: &str) -> Option<String> {
    if v.len() != 6 || !v.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (yy, mm, dd) = (&v[..2], &v[2..4], &v[4..]);
    if !(1..=12).contains(&mm.parse::<u32>().ok()?) {
        return None;
    }
    if dd == "00" {
        Some(format!("20{yy}-{mm}"))
    } else if (1..=31).contains(&dd.parse::<u32>().ok()?) {
        Some(format!("20{yy}-{mm}-{dd}"))
    } else {
        None
    }
}

fn parse_gs1(hrf: &str) -> Gs1 {
    let mut out = Gs1::default();
    for part in hrf.split('(').skip(1) {
        let Some((ai, value)) = part.split_once(')') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match ai {
            "11" => out.manufacture = gs1_date(value),
            "17" => out.expiration = gs1_date(value),
            "10" => out.lot = Some(value.to_owned()),
            "21" => out.serial = Some(value.to_owned()),
            _ => {}
        }
    }
    out
}

fn udi_of(p: &Participant) -> Option<String> {
    p.ids
        .iter()
        .find(|i| i.root.as_deref() == Some(UDI_ROOT))
        .and_then(|i| i.extension.clone())
}

/// The device identifier (DI) of a GS1 UDI written as `(01)<14 digits>(...)`.
fn device_identifier(hrf: &str) -> Option<&str> {
    let rest = hrf.strip_prefix("(01)")?;
    let di = rest.get(..14)?;
    di.bytes().all(|b| b.is_ascii_digit()).then_some(di)
}

fn map_device(
    p: &Participant,
    hrf: Option<String>,
    udi_expected: bool,
    id: String,
    ctx: &mut Context,
) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Device"));
    r.insert("id".into(), json!(id));
    match hrf {
        Some(h) => {
            let gs1 = parse_gs1(&h);
            for (key, value) in [
                ("manufactureDate", gs1.manufacture),
                ("expirationDate", gs1.expiration),
                ("lotNumber", gs1.lot),
                ("serialNumber", gs1.serial),
            ] {
                if let Some(v) = value {
                    r.insert(key.into(), json!(v));
                }
            }
            let mut udi = Map::new();
            if let Some(di) = device_identifier(&h) {
                udi.insert("deviceIdentifier".into(), json!(di));
            }
            udi.insert("carrierHRF".into(), json!(h));
            r.insert("udiCarrier".into(), json!([udi]));
        }
        // Implantable devices carry a UDI; a cane or eyeglasses does not need one.
        None if udi_expected => ctx.report.warn(
            "device.udi.missing",
            "a Product Instance has no UDI; the Device has no udiCarrier",
        ),
        None => {}
    }
    if let Some(ty) = p
        .playing_device_code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
    {
        r.insert("type".into(), ty);
    } else {
        ctx.report.warn(
            "device.type.missing",
            "a Product Instance has no device code; the Device has no type",
        );
    }
    r.insert("patient".into(), json!({ "reference": ctx.patient }));
    Value::Object(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(body: &str) -> (Vec<Value>, Report) {
        let xml = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3">
<component><structuredBody><component><section>
  <code code="46264-8" codeSystem="2.16.840.1.113883.6.1"/>
  {body}
</section></component></structuredBody></component></ClinicalDocument>"#
        );
        let d = ccda_model::parse(xml.as_bytes()).unwrap();
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        (map_devices(&d, &mut ctx), report)
    }

    fn procedure(udi_extension: &str) -> String {
        format!(
            r#"<entry><procedure classCode="PROC" moodCode="EVN">
  <code code="1" codeSystem="2.16.840.1.113883.6.96"/>
  <participant typeCode="DEV"><participantRole classCode="MANU">
    <templateId root="2.16.840.1.113883.10.20.22.4.37"/>
    <id root="2.16.840.1.113883.3.3719" extension="{udi_extension}"/>
    <playingDevice><code code="14106009" codeSystem="2.16.840.1.113883.6.96"/></playingDevice>
  </participantRole></participant>
</procedure></entry>"#
        )
    }

    #[test]
    fn gs1_fields_are_read() {
        let g = parse_gs1("(01)00643169001763(11)170101(17)180400(10)LOT7(21)SN-9");
        assert_eq!(g.manufacture.as_deref(), Some("2017-01-01"));
        assert_eq!(g.expiration.as_deref(), Some("2018-04"));
        assert_eq!(g.lot.as_deref(), Some("LOT7"));
        assert_eq!(g.serial.as_deref(), Some("SN-9"));
        assert_eq!(gs1_date("181332"), None);
        assert_eq!(parse_gs1("not a udi"), Gs1::default());
    }

    #[test]
    fn udi_gives_carrier_with_device_identifier_and_type() {
        let (out, _) = run(&procedure("(01)00643169001763(17)180421(21)ABC"));
        assert_eq!(out.len(), 1);
        let udi = &out[0]["udiCarrier"][0];
        assert_eq!(udi["deviceIdentifier"], "00643169001763");
        assert_eq!(udi["carrierHRF"], "(01)00643169001763(17)180421(21)ABC");
        assert_eq!(out[0]["type"]["coding"][0]["code"], "14106009");
        assert!(out[0]["patient"]["reference"].is_string());
    }

    #[test]
    fn the_same_udi_twice_is_one_device() {
        let both = procedure("(01)00643169001763") + &procedure("(01)00643169001763");
        let (out, _) = run(&both);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn non_gs1_udi_keeps_the_carrier_only() {
        let (out, _) = run(&procedure("+H123PARTNO1234567890"));
        assert!(out[0]["udiCarrier"][0].get("deviceIdentifier").is_none());
        assert_eq!(
            out[0]["udiCarrier"][0]["carrierHRF"],
            "+H123PARTNO1234567890"
        );
    }

    #[test]
    fn missing_udi_is_reported() {
        let xml = r#"<entry><procedure classCode="PROC" moodCode="EVN">
  <participant typeCode="DEV"><participantRole>
    <templateId root="2.16.840.1.113883.10.20.22.4.37"/>
    <playingDevice><code code="14106009" codeSystem="2.16.840.1.113883.6.96"/></playingDevice>
  </participantRole></participant></procedure></entry>"#;
        let (out, report) = run(xml);
        assert!(out[0].get("udiCarrier").is_none());
        assert!(report.issues.iter().any(|i| i.code == "device.udi.missing"));
    }
}
