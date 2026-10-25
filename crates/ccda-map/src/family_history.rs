//! Family History Organizer (4.45) to FHIR `FamilyMemberHistory`.
//!
//! One organizer is one relative, and the Family History Observations (4.46) in it are that
//! relative's conditions. See `docs/decisions/0028-family-history.md`.

use ccda_model::{ClinicalDocument, Link, Statement, StatementKind, Value as Cv};
use serde_json::{json, Map, Value};

use crate::author::{apply_authors, AuthorFields};
use crate::util::nested;
use crate::{map_age, map_codeable_concept, map_datetime, map_identifier, Context};

const FAMILY_HISTORY_ORGANIZER: &str = "2.16.840.1.113883.10.20.22.4.45";
const FAMILY_HISTORY_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.46";
const AGE_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.31";
const DEATH_OBSERVATION: &str = "2.16.840.1.113883.10.20.22.4.47";

const ROLE_CODE_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/v3-RoleCode";
const GENDER_SYSTEM: &str = "http://hl7.org/fhir/administrative-gender";

/// `FamilyMemberHistory.date` is when the history was recorded: the earliest author time.
const FIELDS: AuthorFields = AuthorFields {
    recorder: None,
    date: "date",
};

/// Map every Family History Organizer in the document.
pub fn map_family_history(doc: &ClinicalDocument, ctx: &mut Context) -> Vec<Value> {
    let mut out = Vec::new();
    for section in doc.all_sections() {
        for entry in &section.entries {
            let s = &entry.statement;
            if s.kind == StatementKind::Organizer && s.has_template(FAMILY_HISTORY_ORGANIZER) {
                out.push(map_organizer(s, ctx));
            }
        }
    }
    out
}

/// A date of birth or death as a FHIR `date` (the day, month or year the document gave).
fn date_of(raw: &str, ctx: &mut Context) -> Option<String> {
    let full = map_datetime(raw, ctx.report)?;
    Some(full.split('T').next().unwrap_or(&full).to_owned())
}

fn status(s: &Statement) -> &'static str {
    match s.status_code.as_deref() {
        Some("nullified") => "entered-in-error",
        Some("completed") | None => "completed",
        _ => "partial",
    }
}

fn map_organizer(s: &Statement, ctx: &mut Context) -> Value {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("FamilyMemberHistory"));
    r.insert("id".into(), json!(ctx.next_id("familymemberhistory")));
    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|i| map_identifier(i, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    r.insert("status".into(), json!(status(s)));
    r.insert("patient".into(), json!({ "reference": ctx.patient }));

    let subject = s.subject.as_ref();
    // The relationship is required. A generic "family member" (FAMMEMB) is a real answer; no code
    // at all is marked unknown.
    let relationship = subject
        .and_then(|j| j.code.as_ref())
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report));
    r.insert(
        "relationship".into(),
        relationship.unwrap_or_else(|| {
            ctx.report.warn(
                "familyhistory.relationship.missing",
                "a family history has no relationship code; relationship was marked unknown",
            );
            json!({ "coding": [{ "system": ROLE_CODE_SYSTEM, "code": "FAMMEMB" }] })
        }),
    );

    if let Some(j) = subject {
        if let Some(name) = &j.name {
            r.insert("name".into(), json!(name));
        }
        if let Some(sex) = j.gender_code.as_deref().and_then(|g| match g {
            "M" => Some("male"),
            "F" => Some("female"),
            "UN" => Some("unknown"),
            _ => None,
        }) {
            r.insert(
                "sex".into(),
                json!({ "coding": [{ "system": GENDER_SYSTEM, "code": sex }] }),
            );
        }
        if let Some(born) = j.birth_time.as_deref().and_then(|b| date_of(b, ctx)) {
            r.insert("bornDate".into(), json!(born));
        }
        if let Some(died) = j.deceased_time.as_deref().and_then(|d| date_of(d, ctx)) {
            r.insert("deceasedDate".into(), json!(died));
        } else if let Some(deceased) = j.deceased_ind {
            r.insert("deceasedBoolean".into(), json!(deceased));
        }
        let ids: Vec<Value> = j
            .ids
            .iter()
            .filter_map(|i| map_identifier(i, ctx.terminology, ctx.report))
            .collect();
        if !ids.is_empty() {
            if r.contains_key("identifier") {
                // The history's own identifier wins; FamilyMemberHistory has no place for the
                // relative's id, so it is reported rather than mixed into the history's.
                ctx.report.info(
                    "familyhistory.subject-id.not-mapped",
                    "a relative has an id of their own; it was not mapped because the history already has an identifier",
                );
            } else {
                r.insert("identifier".into(), json!(ids));
            }
        }
    }

    let mut conditions: Vec<Value> = Vec::new();
    let mut notes: Vec<Value> = Vec::new();
    for rel in &s.related {
        if rel.link != Link::Component {
            continue;
        }
        let o = &rel.statement;
        if o.kind != StatementKind::Observation || !o.has_template(FAMILY_HISTORY_OBSERVATION) {
            continue;
        }
        map_condition(o, ctx, &mut conditions, &mut notes);
    }
    if !conditions.is_empty() {
        r.insert("condition".into(), json!(conditions));
    }
    if !notes.is_empty() {
        r.insert("note".into(), json!(notes));
    }
    let authors: Vec<_> = s.authors.iter().collect();
    apply_authors(&mut r, FIELDS, &authors, ctx);
    Value::Object(r)
}

fn map_condition(
    o: &Statement,
    ctx: &mut Context,
    conditions: &mut Vec<Value>,
    notes: &mut Vec<Value>,
) {
    let Some(Cv::Coded(value)) = o.values.first() else {
        ctx.report.info(
            "familyhistory.condition.no-code",
            "a family history observation has no coded value; it was not mapped",
        );
        return;
    };
    let Some(code) = map_codeable_concept(value, ctx.terminology, ctx.report) else {
        ctx.report.info(
            "familyhistory.condition.no-code",
            "a family history observation has no coded value; it was not mapped",
        );
        return;
    };
    // FamilyMemberHistory has no way to say a relative did NOT have a condition, so the statement
    // is kept as a note and reported rather than turned into a positive condition.
    if o.negation_ind {
        ctx.report.info(
            "familyhistory.condition.negated",
            "a relative is stated not to have a condition; it was kept as a note, not a condition",
        );
        let label = value
            .original_text
            .clone()
            .or_else(|| value.display_name.clone())
            .or_else(|| value.code.clone())
            .unwrap_or_default();
        notes.push(json!({ "text": format!("Does not have: {label}") }));
        return;
    }
    let mut c = Map::new();
    c.insert("code".into(), code);
    for n in nested(o) {
        if n.has_template(AGE_OBSERVATION) {
            if let Some(Cv::Quantity(q)) = n.values.first() {
                if let Some(age) = map_age(q, ctx.report) {
                    c.insert("onsetAge".into(), age);
                }
            }
        } else if n.has_template(DEATH_OBSERVATION) {
            c.insert("contributedToDeath".into(), json!(true));
        }
    }
    conditions.push(Value::Object(c));
}
