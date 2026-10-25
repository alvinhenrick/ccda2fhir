//! Find source data that does not show up in the output.
//!
//! Usage: `cargo run --release -p ccda2fhir --example audit -- <file.xml>...`
//!
//! For every attribute value and text value in a source document (outside section narrative), checks
//! whether the converted bundle contains it somewhere. A value that is missing is a candidate for
//! data we drop. Results are grouped by element path and attribute name, with counts only: no
//! document value is ever printed. Some misses are fine (a status mapped to another vocabulary,
//! structural attributes are skipped); the ranking shows where to look first.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::collections::BTreeMap;

use ccda2fhir::{convert, Options};
use ccda_terminology::{BuiltinTerminology, Terminology};
use roxmltree::Node;

/// Attributes that describe structure or map to another vocabulary, so a verbatim match is not
/// expected.
const SKIP_ATTRS: &[&str] = &[
    "classCode",
    "moodCode",
    "typeCode",
    "xsi:type",
    "type",
    "negationInd",
    "inversionInd",
    "contextConductionInd",
    "use",
    "qualifier",
    "nullFlavor",
    "mediaType",
    "representation",
    "ID",
    "language",
    // The name of a code system is redundant once its OID is a URI.
    "codeSystemName",
];
/// Elements that are structure or narrative.
const SKIP_ELEMENTS: &[&str] = &["templateId", "realmCode", "typeId", "text", "statusCode"];

fn leaves(v: &serde_json::Value, out: &mut String) {
    match v {
        serde_json::Value::String(s) => {
            out.push_str(&s.to_lowercase());
            out.push('\n');
        }
        serde_json::Value::Number(n) => {
            out.push_str(&n.to_string());
            out.push('\n');
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| leaves(x, out)),
        serde_json::Value::Object(o) => o.values().for_each(|x| leaves(x, out)),
        _ => {}
    }
}

fn present(haystack: &str, value: &str) -> bool {
    let v = value.trim().to_lowercase();
    if v.is_empty() || haystack.contains(&v) {
        return true;
    }
    // HL7 timestamps become ISO dates and times.
    let digits: String = v.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() >= 4 && digits.len() <= 14 {
        let mut iso = digits[..4].to_owned();
        if digits.len() >= 6 {
            iso = format!("{iso}-{}", &digits[4..6]);
        }
        if digits.len() >= 8 {
            iso = format!("{iso}-{}", &digits[6..8]);
        }
        return haystack.contains(&iso);
    }
    false
}

fn walk(
    node: Node,
    path: &str,
    haystack: &str,
    stats: &mut BTreeMap<String, (usize, usize)>,
    narrative: &mut usize,
) {
    for child in node.children().filter(Node::is_element) {
        let name = child.tag_name().name();
        if name == "text" && node.tag_name().name() == "section" {
            *narrative += 1;
            continue;
        }
        if SKIP_ELEMENTS.contains(&name) {
            continue;
        }
        let here = format!("{path}/{name}");
        let short: String = {
            let parts: Vec<&str> = here.split('/').filter(|p| !p.is_empty()).collect();
            parts[parts.len().saturating_sub(4)..].join("/")
        };
        for a in child.attributes() {
            let key = a.name();
            if SKIP_ATTRS.contains(&key) || a.value().is_empty() {
                continue;
            }
            let entry = stats.entry(format!("{short}@{key}")).or_default();
            entry.0 += 1;
            let found = present(haystack, a.value())
                || (matches!(key, "codeSystem" | "root")
                    && present(haystack, &BuiltinTerminology.system_for_oid(a.value())));
            if !found {
                entry.1 += 1;
            }
        }
        let direct: String = child
            .children()
            .filter(Node::is_text)
            .filter_map(|t| t.text())
            .collect::<String>();
        if !direct.trim().is_empty() {
            let entry = stats.entry(format!("{short}#text")).or_default();
            entry.0 += 1;
            if !present(haystack, &direct) {
                entry.1 += 1;
            }
        }
        walk(child, &here, haystack, stats, narrative);
    }
}

fn main() {
    let mut stats: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let (mut files, mut narrative) = (0usize, 0usize);
    for path in std::env::args().skip(1) {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(c) = convert(&bytes, &Options::default()) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let Ok(doc) = roxmltree::Document::parse_with_options(
            text,
            roxmltree::ParsingOptions {
                allow_dtd: false,
                ..Default::default()
            },
        ) else {
            continue;
        };
        files += 1;
        let mut haystack = String::new();
        leaves(&c.bundle.to_json(), &mut haystack);
        walk(doc.root(), "", &haystack, &mut stats, &mut narrative);
    }
    let total: usize = stats.values().map(|s| s.0).sum();
    let missing: usize = stats.values().map(|s| s.1).sum();
    println!(
        "{files} documents; {total} values checked, {missing} not found in the output ({} section narratives skipped)\n",
        narrative
    );
    let mut rows: Vec<_> = stats.iter().filter(|(_, s)| s.1 > 0).collect();
    rows.sort_by(|a, b| b.1 .1.cmp(&a.1 .1).then(a.0.cmp(b.0)));
    println!("{:>6} {:>6}  path", "missing", "total");
    for (k, (t, m)) in rows.iter().take(80).map(|(k, s)| (k, (s.0, s.1))) {
        println!("{m:>6} {t:>6}  {k}");
    }
}
