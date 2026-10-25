//! Link clinical resources to the Encounter they happened in.
//!
//! In C-CDA an entry names its encounter with a nested Encounter Activity (4.49) that carries only
//! the encounter's id. We resolve that id against the Encounters mapped from the document and set
//! `encounter` on the resource made from the entry. Matching is by identifier, so an entry
//! without an id cannot be linked; an encounter that is not in the document is reported. See
//! `docs/decisions/0025-encounter-links.md`.

use std::collections::BTreeMap;

use ccda_model::{ClinicalDocument, Statement, StatementKind};
use serde_json::{json, Value};

use crate::health_concern::keys;
use crate::{map_identifier, Context};

/// Resource types that have an `encounter` element in R4.
const LINKABLE: &[&str] = &[
    "Condition",
    "Observation",
    "Procedure",
    "DiagnosticReport",
    "MedicationRequest",
    "Immunization",
    "ServiceRequest",
    "CarePlan",
];

pub(crate) fn statement_keys(s: &Statement, ctx: &mut Context) -> Vec<String> {
    let ids: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|i| map_identifier(i, ctx.terminology, ctx.report))
        .collect();
    keys(&json!(ids))
}

/// Set `encounter` on every resource whose entry names an encounter that was mapped.
pub fn link_encounters(doc: &ClinicalDocument, ctx: &mut Context, resources: &mut [Value]) {
    let mut encounters: BTreeMap<String, String> = BTreeMap::new();
    let mut targets: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, r) in resources.iter().enumerate() {
        let type_ = r["resourceType"].as_str().unwrap_or_default();
        if type_ == "Encounter" {
            if let Some(id) = r["id"].as_str() {
                for k in keys(&r["identifier"]) {
                    encounters
                        .entry(k)
                        .or_insert_with(|| ctx.reference("Encounter", id));
                }
            }
        } else if LINKABLE.contains(&type_) {
            for k in keys(&r["identifier"]) {
                targets.entry(k).or_default().push(i);
            }
        }
    }
    if encounters.is_empty() && targets.is_empty() {
        return;
    }
    let mut work: Vec<&Statement> = doc
        .all_sections()
        .into_iter()
        .flat_map(|s| s.entries.iter().map(|e| &e.statement))
        .collect();
    while let Some(s) = work.pop() {
        work.extend(s.related.iter().map(|r| &r.statement));
        if s.kind == StatementKind::Encounter {
            continue;
        }
        let Some(named) = s
            .related
            .iter()
            .map(|r| &r.statement)
            .find(|n| n.kind == StatementKind::Encounter && !n.ids.is_empty())
        else {
            continue;
        };
        let reference = statement_keys(named, ctx)
            .into_iter()
            .find_map(|k| encounters.get(&k).cloned());
        let Some(reference) = reference else {
            ctx.report.info(
                "encounter.link.unresolved",
                "an entry names an encounter that is not in the document; no link was made",
            );
            continue;
        };
        for k in statement_keys(s, ctx) {
            for &i in targets.get(&k).into_iter().flatten() {
                if resources[i].get("encounter").is_none() {
                    resources[i]["encounter"] = json!({ "reference": reference });
                }
            }
        }
    }
}
