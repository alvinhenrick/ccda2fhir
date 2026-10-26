//! Generated narrative (`Resource.text`) for resources that have none.
//!
//! Clinical resources should carry a human readable summary (the best-practice rule `dom-6`).
//! The text is built only from values already in the resource, in a fixed field order, so it is
//! deterministic and adds nothing the structured data does not say.

use serde_json::{json, Value};

/// Fields summarised, in order. Labels are the FHIR element names.
const FIELDS: &[&str] = &[
    "name",
    "code",
    "type",
    "vaccineCode",
    "medicationCodeableConcept",
    "category",
    "status",
    "clinicalStatus",
    "description",
    "title",
    "onsetDateTime",
    "effectiveDateTime",
    "performedDateTime",
    "occurrenceDateTime",
    "authoredOn",
    "issued",
    "date",
    "birthDate",
    "gender",
    "valueQuantity",
    "valueString",
    "valueCodeableConcept",
];

const SKIP: &[&str] = &["Bundle", "Binary", "Parameters"];

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A short plain-text rendering of a value, or `None` if there is nothing sensible to show.
fn render(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()).filter(|s| !s.trim().is_empty()),
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().filter_map(render).collect();
            Some(parts.join("; ")).filter(|s| !s.is_empty())
        }
        Value::Object(o) => {
            if let Some(t) = o.get("text").and_then(Value::as_str) {
                return Some(t.to_owned());
            }
            if let Some(codings) = o.get("coding").and_then(Value::as_array) {
                return codings.iter().find_map(|c| {
                    c.get("display")
                        .or_else(|| c.get("code"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                });
            }
            if let Some(n) = o.get("value").filter(|n| n.is_number()) {
                let unit = o
                    .get("unit")
                    .or_else(|| o.get("code"))
                    .and_then(Value::as_str);
                return Some(match unit {
                    Some(u) => format!("{n} {u}"),
                    None => n.to_string(),
                });
            }
            if o.contains_key("family") || o.contains_key("given") {
                let mut parts: Vec<&str> = o
                    .get("given")
                    .and_then(Value::as_array)
                    .map(|g| g.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                if let Some(f) = o.get("family").and_then(Value::as_str) {
                    parts.push(f);
                }
                return Some(parts.join(" ")).filter(|s| !s.is_empty());
            }
            o.get("display").and_then(Value::as_str).map(str::to_owned)
        }
        _ => None,
    }
}

/// When a resource has a `language`, every narrative `div` in it (its own and its sections')
/// carries the same `lang` and `xml:lang`, which FHIR asks for.
pub fn mark_narrative_language(resource: &mut Value) {
    let Some(language) = resource["language"].as_str().map(str::to_owned) else {
        return;
    };
    let escaped = language
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;");
    let from = "<div xmlns=\"http://www.w3.org/1999/xhtml\">";
    let to = format!(
        "<div xmlns=\"http://www.w3.org/1999/xhtml\" lang=\"{escaped}\" xml:lang=\"{escaped}\">"
    );
    fn walk(v: &mut Value, from: &str, to: &str) {
        match v {
            Value::Object(map) => {
                if let Some(Value::String(div)) = map.get_mut("div") {
                    if div.starts_with(from) {
                        *div = format!("{to}{}", &div[from.len()..]);
                    }
                }
                for child in map.values_mut() {
                    walk(child, from, to);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|i| walk(i, from, to)),
            _ => {}
        }
    }
    walk(resource, from, &to);
}

/// Add a generated `text` to `resource` unless it already has one.
pub fn add_generated_text(resource: &mut Value) {
    let Some(kind) = resource["resourceType"].as_str().map(str::to_owned) else {
        return;
    };
    if SKIP.contains(&kind.as_str()) || resource.get("text").is_some() {
        return;
    }
    let mut rows = String::new();
    for field in FIELDS {
        if let Some(text) = resource.get(*field).and_then(render) {
            rows.push_str(&format!(
                "<tr><td>{}</td><td>{}</td></tr>",
                escape(field),
                escape(&text)
            ));
        }
    }
    let body = if rows.is_empty() {
        format!("<p>{}</p>", escape(&kind))
    } else {
        format!("<p>{}</p><table>{rows}</table>", escape(&kind))
    };
    resource["text"] = json!({
        "status": "generated",
        "div": format!("<div xmlns=\"http://www.w3.org/1999/xhtml\">{body}</div>"),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarises_known_fields_and_escapes() {
        let mut r = json!({
            "resourceType": "Condition",
            "code": { "coding": [{ "display": "Asthma <chronic>" }] },
            "clinicalStatus": { "coding": [{ "code": "active" }] },
        });
        add_generated_text(&mut r);
        let div = r["text"]["div"].as_str().unwrap();
        assert_eq!(r["text"]["status"], "generated");
        assert!(div.contains("Asthma &lt;chronic&gt;"));
        assert!(div.contains("<td>clinicalStatus</td><td>active</td>"));
    }

    #[test]
    fn existing_text_is_kept() {
        let mut r =
            json!({ "resourceType": "CarePlan", "text": { "status": "additional", "div": "x" } });
        add_generated_text(&mut r);
        assert_eq!(r["text"]["div"], "x");
    }

    #[test]
    fn empty_resource_still_gets_a_valid_div() {
        let mut r = json!({ "resourceType": "Organization", "id": "o1" });
        add_generated_text(&mut r);
        assert!(r["text"]["div"]
            .as_str()
            .unwrap()
            .contains("<p>Organization</p>"));
    }

    #[test]
    fn names_and_quantities_render() {
        let mut r = json!({
            "resourceType": "Observation",
            "valueQuantity": { "value": 5.5, "unit": "mg" },
            "name": [{ "given": ["A"], "family": "B" }],
        });
        add_generated_text(&mut r);
        let div = r["text"]["div"].as_str().unwrap();
        assert!(div.contains("5.5 mg"));
        assert!(div.contains("A B"));
    }
}
