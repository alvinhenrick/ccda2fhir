//! Result specimens to FHIR `Specimen`.
//!
//! Rules come from the IG page "CF-results" (C-CDA Specimen to FHIR Specimen).
//! See `docs/decisions/0018-location-and-specimen.md`.

use ccda_model::{Specimen, Statement};
use serde_json::{json, Map, Value};

use crate::quantity::map_quantity;
use crate::{map_codeable_concept, map_datetime, map_identifier, Context};

const SPECIMEN_COLLECTION: &str = "17636008";

/// Create (or reuse) a `Specimen` for each specimen role; return their references.
///
/// `collection` is the organizer's Specimen Collection Procedure, whose target site and time
/// describe how the specimen was taken.
pub(crate) fn register_specimens(
    specimens: &[Specimen],
    collection: Option<&Statement>,
    ctx: &mut Context,
) -> Vec<String> {
    specimens
        .iter()
        .filter_map(|s| register(s, collection, ctx))
        .collect()
}

/// The Specimen Collection Procedure among an organizer's components, if any.
pub(crate) fn collection_procedure(org: &Statement) -> Option<&Statement> {
    org.related.iter().map(|r| &r.statement).find(|s| {
        s.code
            .as_ref()
            .is_some_and(|c| c.code.as_deref() == Some(SPECIMEN_COLLECTION))
    })
}

fn register(s: &Specimen, collection: Option<&Statement>, ctx: &mut Context) -> Option<String> {
    let key = s.ids.iter().find_map(|id| {
        Some(format!(
            "id:{}|{}",
            id.root.as_deref()?,
            id.extension.as_deref()?
        ))
    });
    if let Some(existing) = key.as_ref().and_then(|k| ctx.specimens.get(k)) {
        return Some(existing.clone());
    }

    let coded = s
        .code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report));
    let name = s.name.as_deref().filter(|n| !n.trim().is_empty());
    let mut notes = Vec::new();
    let type_ = match (coded, name) {
        (Some(mut t), n) => {
            if t["text"].is_null() {
                if let Some(n) = n {
                    t["text"] = json!(n);
                }
            } else if let Some(n) = n.filter(|n| Some(*n) != t["text"].as_str()) {
                notes.push(json!({ "text": n }));
            }
            Some(t)
        }
        (None, Some(n)) => Some(json!({ "text": n })),
        (None, None) => None,
    };
    if type_.is_none() && s.ids.is_empty() {
        ctx.report.info(
            "specimen.empty",
            "a specimen has no code, name or id; it was not mapped",
        );
        return None;
    }

    let id = ctx.next_id("specimen");
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Specimen"));
    r.insert("id".into(), json!(id));
    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|i| map_identifier(i, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    if let Some(t) = type_ {
        r.insert("type".into(), t);
    }
    r.insert("subject".into(), json!({ "reference": ctx.patient }));

    let mut coll = Map::new();
    if let Some(q) = s
        .quantity
        .as_ref()
        .and_then(|q| map_quantity(q, ctx.report))
    {
        coll.insert("quantity".into(), q);
    }
    if let Some(proc) = collection {
        if let Some(site) = proc
            .target_site_codes
            .first()
            .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
        {
            coll.insert("bodySite".into(), site);
        }
        let t = proc.effective_time.as_ref();
        let value = t.and_then(|t| t.value.as_deref()).or_else(|| {
            t.and_then(|t| t.low.as_ref())
                .and_then(|p| p.value.as_deref())
        });
        if let Some(d) = value.and_then(|v| map_datetime(v, ctx.report)) {
            coll.insert("collectedDateTime".into(), json!(d));
        }
    }
    if !coll.is_empty() {
        r.insert("collection".into(), Value::Object(coll));
    }
    if let Some(d) = s.description.as_deref().filter(|d| !d.trim().is_empty()) {
        notes.push(json!({ "text": d }));
    }
    if !notes.is_empty() {
        r.insert("note".into(), json!(notes));
    }

    let reference = ctx.reference("Specimen", &id);
    if let Some(k) = key {
        ctx.specimens.insert(k, reference.clone());
    }
    ctx.push_extra(Value::Object(r));
    Some(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_terminology::BuiltinTerminology;

    fn run(specs: &[Specimen]) -> (Vec<String>, Vec<Value>, Report) {
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let refs = register_specimens(specs, None, &mut ctx);
        let extra = ctx.take_extra();
        (refs, extra, report)
    }

    fn named(n: &str) -> Specimen {
        Specimen {
            name: Some(n.into()),
            ..Specimen::default()
        }
    }

    #[test]
    fn name_only_specimen_gets_text_type_and_subject() {
        let (refs, extra, _) = run(&[named("Urine")]);
        assert_eq!(refs.len(), 1);
        assert_eq!(extra[0]["type"]["text"], "Urine");
        assert!(extra[0]["subject"]["reference"].is_string());
    }

    #[test]
    fn description_becomes_a_note() {
        let mut s = named("Urine");
        s.description = Some("Midstream".into());
        let (_, extra, _) = run(&[s]);
        assert_eq!(extra[0]["note"][0]["text"], "Midstream");
    }

    #[test]
    fn empty_specimen_is_reported() {
        let (refs, extra, report) = run(&[Specimen::default()]);
        assert!(refs.is_empty() && extra.is_empty());
        assert!(report.issues.iter().any(|i| i.code == "specimen.empty"));
    }
}
