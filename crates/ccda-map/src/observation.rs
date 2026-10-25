//! Building blocks shared by every Observation-based mapper (vital signs, results, social
//! history, ...): status, effective time, value, interpretation, method, site, reference range
//! and performer. Rules come from the IG pages "CF-vitals" and "CF-results".

use ccda_model::{Quantity, Statement, Value as Cv};
use serde_json::{json, Map, Value};

use crate::author::{apply_authors, AuthorFields};
use crate::{
    data_absent_reason_extension, decimal, map_codeable_concept, map_datetime, map_identifier,
    map_quantity, Context,
};

pub(crate) const OBSERVATION_CATEGORY: &str =
    "http://terminology.hl7.org/CodeSystem/observation-category";

/// `CF-ResultStatus`: v3 ActStatus to observation-status. `nullified` is not in the IG's map; it is
/// the only sensible reading of "entered in error".
pub(crate) fn status_of(code: &str) -> Option<&'static str> {
    Some(match code {
        "completed" => "final",
        "active" | "held" | "suspended" => "registered",
        "aborted" | "cancelled" => "cancelled",
        "nullified" => "entered-in-error",
        _ => return None,
    })
}

pub(crate) fn status(s: &Statement, ctx: &mut Context) -> &'static str {
    match s.status_code.as_deref() {
        Some(code) => status_of(code).unwrap_or_else(|| {
            ctx.report.warn(
                "observation.status.unmapped",
                "a statusCode is not in the IG's status map; status was set to unknown",
            );
            "unknown"
        }),
        None => {
            ctx.report.warn(
                "observation.status.missing",
                "an observation has no statusCode; status was set to unknown",
            );
            "unknown"
        }
    }
}

pub(crate) fn category(code: &str, display: &str) -> Value {
    json!([{ "coding": [{ "system": OBSERVATION_CATEGORY, "code": code, "display": display }] }])
}

fn dar(nf: &str, ctx: &mut Context) -> Value {
    json!({ "extension": [data_absent_reason_extension(nf, ctx.report)] })
}

fn range(low: &Option<Quantity>, high: &Option<Quantity>, ctx: &mut Context) -> Option<Value> {
    let mut out = Map::new();
    for (key, q) in [("low", low), ("high", high)] {
        if let Some(v) = q.as_ref().and_then(|q| map_quantity(q, ctx.report)) {
            out.insert(key.into(), v);
        }
    }
    (!out.is_empty()).then_some(Value::Object(out))
}

/// Add `effective[x]` from the statement's time: a point, or a period (a single instant when
/// `low` equals `high`).
pub(crate) fn insert_effective(r: &mut Map<String, Value>, s: &Statement, ctx: &mut Context) {
    let Some(t) = s.effective_time.as_ref() else {
        return;
    };
    let low = t.low.as_ref().and_then(|p| p.value.as_deref());
    let high = t.high.as_ref().and_then(|p| p.value.as_deref());
    if let Some(v) = t.value.as_deref() {
        if let Some(d) = map_datetime(v, ctx.report) {
            r.insert("effectiveDateTime".into(), json!(d));
        }
        return;
    }
    let low = low.and_then(|v| map_datetime(v, ctx.report));
    let high = high.and_then(|v| map_datetime(v, ctx.report));
    match (low, high) {
        (Some(l), Some(h)) if l == h => {
            r.insert("effectiveDateTime".into(), json!(l));
        }
        (None, None) => {}
        (l, h) => {
            let mut p = Map::new();
            if let Some(l) = l {
                p.insert("start".into(), json!(l));
            }
            if let Some(h) = h {
                p.insert("end".into(), json!(h));
            }
            r.insert("effectivePeriod".into(), Value::Object(p));
        }
    }
}

/// Add `value[x]` (or a data absent reason) from the statement's first value.
pub(crate) fn insert_value(r: &mut Map<String, Value>, s: &Statement, ctx: &mut Context) {
    let Some(value) = s.values.first() else {
        return;
    };
    if s.values.len() > 1 {
        ctx.report.info(
            "observation.value.multiple",
            "an observation has more than one value; only the first was mapped",
        );
    }
    match value {
        Cv::Quantity(q) => {
            if let Some(v) = map_quantity(q, ctx.report) {
                r.insert("valueQuantity".into(), v);
            } else if let Some(nf) = q.null_flavor.as_deref() {
                r.insert("dataAbsentReason".into(), dar_concept(nf, ctx));
            }
        }
        Cv::Coded(c) => {
            if let Some(v) = map_codeable_concept(c, ctx.terminology, ctx.report) {
                r.insert("valueCodeableConcept".into(), v);
            } else if let Some(nf) = c.null_flavor.as_deref() {
                r.insert("dataAbsentReason".into(), dar_concept(nf, ctx));
            }
        }
        Cv::Integer(x) => match x.value.as_deref().and_then(|v| v.parse::<i64>().ok()) {
            Some(n) => {
                r.insert("valueInteger".into(), json!(n));
            }
            None => absent_scalar(r, x.null_flavor.as_deref(), ctx),
        },
        Cv::Real(x) => match x.value.as_deref().and_then(decimal) {
            Some(n) => {
                r.insert("valueQuantity".into(), json!({ "value": n }));
            }
            None => absent_scalar(r, x.null_flavor.as_deref(), ctx),
        },
        Cv::Text(x) => match x.value.as_deref().filter(|v| !v.trim().is_empty()) {
            Some(t) => {
                r.insert("valueString".into(), json!(t));
            }
            None => absent_scalar(r, x.null_flavor.as_deref(), ctx),
        },
        Cv::Boolean(x) => match x.value.as_deref() {
            Some("true") => {
                r.insert("valueBoolean".into(), json!(true));
            }
            Some("false") => {
                r.insert("valueBoolean".into(), json!(false));
            }
            _ => absent_scalar(r, x.null_flavor.as_deref(), ctx),
        },
        Cv::Timestamp(x) => match x.value.as_deref().and_then(|v| map_datetime(v, ctx.report)) {
            Some(d) => {
                r.insert("valueDateTime".into(), json!(d));
            }
            None => absent_scalar(r, x.null_flavor.as_deref(), ctx),
        },
        Cv::Range { low, high } => {
            if let Some(v) = range(low, high, ctx) {
                r.insert("valueRange".into(), v);
            }
        }
        Cv::Other { .. } => ctx.report.warn(
            "observation.value.unsupported",
            "an observation value has a type that is not mapped yet and was omitted",
        ),
    }
}

fn dar_concept(nf: &str, ctx: &mut Context) -> Value {
    let ext = data_absent_reason_extension(nf, ctx.report);
    let code = ext["valueCode"].clone();
    json!({ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/data-absent-reason", "code": code }] })
}

fn absent_scalar(r: &mut Map<String, Value>, nf: Option<&str>, ctx: &mut Context) {
    if let Some(nf) = nf {
        r.insert("dataAbsentReason".into(), dar_concept(nf, ctx));
    }
}

/// `interpretation`, `method`, `bodySite` and `referenceRange`: shared by an Observation and by
/// each of its components.
pub(crate) fn insert_details(r: &mut Map<String, Value>, s: &Statement, ctx: &mut Context) {
    let interpretations: Vec<Value> = s
        .interpretation_codes
        .iter()
        .filter_map(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
        .collect();
    if !interpretations.is_empty() {
        r.insert("interpretation".into(), json!(interpretations));
    }
    for (field, codes) in [
        ("method", &s.method_codes),
        ("bodySite", &s.target_site_codes),
    ] {
        if let Some(first) = codes.first() {
            if let Some(c) = map_codeable_concept(first, ctx.terminology, ctx.report) {
                r.insert(field.into(), c);
            }
            if codes.len() > 1 {
                ctx.report.info(
                    "observation.code.multiple",
                    "a method or site has more than one code; only the first was mapped",
                );
            }
        }
    }

    // FHIR reference ranges are "normal" ranges: keep those marked N (or unmarked).
    let mut ranges: Vec<Value> = Vec::new();
    for rr in &s.reference_ranges {
        let normal = rr
            .interpretation
            .as_ref()
            .and_then(|c| c.code.as_deref())
            .is_none_or(|c| c == "N");
        if !normal {
            ctx.report.info(
                "observation.reference-range.not-normal",
                "a reference range that is not a normal range has no FHIR element and was omitted",
            );
            continue;
        }
        let mut out = Map::new();
        match &rr.value {
            Some(Cv::Range { low, high }) => {
                for (key, q) in [("low", low), ("high", high)] {
                    if let Some(v) = q.as_ref().and_then(|q| map_quantity(q, ctx.report)) {
                        out.insert(key.into(), v);
                    }
                }
            }
            Some(Cv::Text(t)) => {
                if let Some(text) = t.value.as_deref().filter(|v| !v.trim().is_empty()) {
                    out.insert("text".into(), json!(text));
                }
            }
            Some(_) => ctx.report.info(
                "observation.reference-range.unsupported",
                "a reference range value has a type that is not mapped and was omitted",
            ),
            None => {}
        }
        if let Some(text) = rr.text.as_deref().filter(|v| !v.trim().is_empty()) {
            out.entry("text").or_insert_with(|| json!(text));
        }
        if !out.is_empty() {
            ranges.push(Value::Object(out));
        }
    }
    if !ranges.is_empty() {
        r.insert("referenceRange".into(), json!(ranges));
    }
}

/// The common start of an Observation: id, identifier, status, code, subject, effective, value
/// and details. The caller adds the category, then calls [`apply_performer`].
pub(crate) fn base(
    s: &Statement,
    id_prefix: &'static str,
    ctx: &mut Context,
) -> Map<String, Value> {
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("Observation"));
    r.insert("id".into(), json!(ctx.next_id(id_prefix)));
    let identifiers: Vec<Value> = s
        .ids
        .iter()
        .filter_map(|id| map_identifier(id, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        r.insert("identifier".into(), json!(identifiers));
    }
    r.insert("status".into(), json!(status(s, ctx)));
    let code = s.code.as_ref().and_then(|c| {
        map_codeable_concept(c, ctx.terminology, ctx.report)
            .or_else(|| c.null_flavor.as_deref().map(|nf| dar(nf, ctx)))
    });
    r.insert(
        "code".into(),
        code.unwrap_or_else(|| {
            ctx.report.warn(
                "observation.code.missing",
                "an observation has no code; code was marked unknown",
            );
            dar("UNK", ctx)
        }),
    );
    r.insert("subject".into(), json!({ "reference": ctx.patient }));
    insert_effective(&mut r, s, ctx);
    insert_value(&mut r, s, ctx);
    insert_details(&mut r, s, ctx);
    r
}

/// Add `performer` (the latest author) and create the `Provenance`. `fallback` supplies the
/// authors of an enclosing organizer when the observation has none.
pub(crate) fn apply_performer(
    r: &mut Map<String, Value>,
    s: &Statement,
    fallback: Option<&Statement>,
    ctx: &mut Context,
) {
    let authors: Vec<_> = if s.authors.is_empty() {
        fallback
            .map(|f| f.authors.iter().collect())
            .unwrap_or_default()
    } else {
        s.authors.iter().collect()
    };
    // Observation has no single recorder or authored-on element: use scratch keys, then move
    // the recorder into the `performer` array and drop the date.
    apply_authors(
        r,
        AuthorFields {
            recorder: Some("_performer"),
            date: "_authored",
        },
        &authors,
        ctx,
    );
    r.remove("_authored");
    if let Some(p) = r.remove("_performer") {
        r.insert("performer".into(), json!([p]));
    }
}

/// Set `effectiveDateTime` or `effectivePeriod` on `r` from the span of the members' own times.
/// Used when an organizer has no time of its own.
pub(crate) fn insert_span(r: &mut Map<String, Value>, members: &[Value]) {
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
