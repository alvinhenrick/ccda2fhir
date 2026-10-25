//! Golden-file tests: convert a synthetic fixture and compare with the checked-in FHIR output.
//!
//! The golden files are the reviewed, expected output. When mapping rules change on purpose,
//! regenerate them with `UPDATE_GOLDEN=1 cargo test -p ccda2fhir --test golden`, then read the
//! diff before committing.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use ccda2fhir::{convert, Options};

fn dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name)
}

fn check(name: &str) {
    let xml = std::fs::read(dir("fixtures").join(format!("{name}.xml"))).unwrap();
    let conversion = convert(&xml, &Options::default()).unwrap();

    let actual = serde_json::json!({
        "bundle": conversion.bundle.to_json(),
        "report": conversion.report.issues.iter().map(|i| serde_json::json!({
            "severity": format!("{:?}", i.severity).to_lowercase(),
            "code": i.code,
        })).collect::<Vec<_>>(),
    });
    let mut text = serde_json::to_string_pretty(&actual).unwrap();
    text.push('\n');

    let golden = dir("golden").join(format!("{name}.json"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, &text).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&golden).unwrap_or_else(|_| {
        panic!(
            "missing golden file {}; run with UPDATE_GOLDEN=1",
            golden.display()
        )
    });
    assert_eq!(
        text, expected,
        "{name}: output differs from the golden file"
    );
}

#[test]
fn problems() {
    check("problems");
}

#[test]
fn allergies() {
    check("allergies");
}

#[test]
fn immunizations() {
    check("immunizations");
}

#[test]
fn medications() {
    check("medications");
}

#[test]
fn vitals() {
    check("vitals");
}

#[test]
fn results() {
    check("results");
}
