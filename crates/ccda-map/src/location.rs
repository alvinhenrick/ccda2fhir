//! Service Delivery Location participants to FHIR `Location`.
//!
//! See `docs/decisions/0018-location-and-specimen.md`.

use ccda_model::Participant;
use serde_json::{json, Map, Value};

use crate::patient::{address, telecom};
use crate::{map_codeable_concept, map_identifier, Context};

/// Create (or reuse) a `Location` for a participant and return its reference.
///
/// A location with no name is not mapped: US Core requires `Location.name`.
pub(crate) fn register_location(p: &Participant, ctx: &mut Context) -> Option<String> {
    let Some(name) = p.name.as_deref().filter(|n| !n.trim().is_empty()) else {
        ctx.report.info(
            "location.name.missing",
            "a service delivery location has no name; it was not mapped",
        );
        return None;
    };
    let key = p
        .ids
        .iter()
        .find_map(|id| {
            Some(format!(
                "id:{}|{}",
                id.root.as_deref()?,
                id.extension.as_deref()?
            ))
        })
        .unwrap_or_else(|| format!("name:{name}"));
    if let Some(existing) = ctx.locations.get(&key) {
        return Some(existing.clone());
    }
    let id = ctx.next_id("location");
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Location"));
    r.insert("id".into(), json!(id));
    let identifiers: Vec<Value> = p
        .ids
        .iter()
        .filter_map(|i| map_identifier(i, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    r.insert("name".into(), json!(name));
    if let Some(t) = p
        .role_code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
    {
        r.insert("type".into(), json!([t]));
    }
    let telecoms: Vec<Value> = p.telecoms.iter().filter_map(telecom).collect();
    if !telecoms.is_empty() {
        r.insert("telecom".into(), json!(telecoms));
    }
    if let Some(a) = p.addresses.iter().find_map(address) {
        r.insert("address".into(), a);
    }
    let reference = ctx.reference("Location", &id);
    ctx.locations.insert(key, reference.clone());
    ctx.push_extra(Value::Object(r));
    Some(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn loc(name: Option<&str>) -> Participant {
        Participant {
            type_code: Some("LOC".into()),
            name: name.map(str::to_owned),
            ..Participant::default()
        }
    }

    #[test]
    fn named_location_is_created_once() {
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let a = register_location(&loc(Some("Main Clinic")), &mut ctx).unwrap();
        let b = register_location(&loc(Some("Main Clinic")), &mut ctx).unwrap();
        assert_eq!(a, b);
        let extra = ctx.take_extra();
        assert_eq!(extra.len(), 1);
        assert_eq!(extra[0]["name"], "Main Clinic");
    }

    #[test]
    fn nameless_location_is_reported_not_mapped() {
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        assert!(register_location(&loc(None), &mut ctx).is_none());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "location.name.missing"));
    }
}
