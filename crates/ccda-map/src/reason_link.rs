//! Link a resource to the Condition that is its reason.
//!
//! An Indication (4.19) nested in a medication, procedure, encounter, immunization or request
//! describes why it was done. When the Indication carries the identifier of a Problem Observation
//! that became a Condition, we also set `reasonReference` on the resource made from the parent
//! entry. `reasonCode` stays, so nothing the document said is lost. Matching is by identifier; an
//! Indication whose id names nothing in the document adds nothing and is not an error. See
//! `docs/decisions/0050-reason-reference.md`.

use std::collections::BTreeMap;

use ccda_model::{ClinicalDocument, Statement};
use serde_json::{json, Value};

use crate::encounter_link::statement_keys;
use crate::health_concern::keys;
use crate::Context;

const INDICATION: &str = "2.16.840.1.113883.10.20.22.4.19";

/// Resource types with a `reasonReference` that can point at a Condition.
const LINKABLE: &[&str] = &[
    "Encounter",
    "MedicationRequest",
    "Procedure",
    "ServiceRequest",
    "Immunization",
];

/// Set `reasonReference` on every resource whose entry has an Indication naming a mapped Condition.
pub fn link_reasons(doc: &ClinicalDocument, ctx: &mut Context, resources: &mut [Value]) {
    let mut conditions: BTreeMap<String, String> = BTreeMap::new();
    let mut targets: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, r) in resources.iter().enumerate() {
        let type_ = r["resourceType"].as_str().unwrap_or_default();
        if type_ == "Condition" {
            if let Some(id) = r["id"].as_str() {
                for k in keys(&r["identifier"]) {
                    conditions
                        .entry(k)
                        .or_insert_with(|| ctx.reference("Condition", id));
                }
            }
        } else if LINKABLE.contains(&type_) {
            for k in keys(&r["identifier"]) {
                targets.entry(k).or_default().push(i);
            }
        }
    }
    if conditions.is_empty() || targets.is_empty() {
        return;
    }
    let mut work: Vec<&Statement> = doc
        .all_sections()
        .into_iter()
        .flat_map(|s| s.entries.iter().map(|e| &e.statement))
        .collect();
    while let Some(s) = work.pop() {
        work.extend(s.related.iter().map(|r| &r.statement));
        let mut refs: Vec<String> = Vec::new();
        for ind in s
            .related
            .iter()
            .map(|r| &r.statement)
            .filter(|n| n.has_template(INDICATION))
        {
            if let Some(c) = statement_keys(ind, ctx)
                .into_iter()
                .find_map(|k| conditions.get(&k).cloned())
            {
                if !refs.contains(&c) {
                    refs.push(c);
                }
            }
        }
        if refs.is_empty() {
            continue;
        }
        for k in statement_keys(s, ctx) {
            for &i in targets.get(&k).into_iter().flatten() {
                let mut list = resources[i]["reasonReference"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                for c in &refs {
                    let v = json!({ "reference": c });
                    if !list.contains(&v) {
                        list.push(v);
                    }
                }
                resources[i]["reasonReference"] = json!(list);
            }
        }
    }
}
