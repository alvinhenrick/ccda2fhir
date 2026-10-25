//! Convert a set of C-CDA files and summarize what happened, without printing document content.
//!
//! Usage: `cargo run --release -p ccda2fhir --example corpus -- <file.xml>...`
//!
//! Prints one line per file (resources, report entries, time) and then totals: resources by type
//! and report codes by frequency. Use it on documents you are allowed to process; nothing from
//! the documents is printed, only counts and the stable report codes.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::collections::BTreeMap;
use std::time::Instant;

use ccda2fhir::{convert, Options};

fn main() {
    let mut types: BTreeMap<String, usize> = BTreeMap::new();
    let mut codes: BTreeMap<(String, &'static str), usize> = BTreeMap::new();
    let mut unmapped: BTreeMap<String, usize> = BTreeMap::new();
    let (mut ok, mut failed) = (0usize, 0usize);
    for path in std::env::args().skip(1) {
        let name = std::path::Path::new(&path)
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let Ok(bytes) = std::fs::read(&path) else {
            println!("{name:<60} unreadable");
            continue;
        };
        let t = Instant::now();
        match convert(&bytes, &Options::default()) {
            Ok(c) => {
                ok += 1;
                let json = c.bundle.to_json();
                let n = json["entry"].as_array().map_or(0, Vec::len);
                for e in json["entry"].as_array().into_iter().flatten() {
                    let t = e["resource"]["resourceType"].as_str().unwrap_or("?");
                    *types.entry(t.to_owned()).or_default() += 1;
                }
                for i in &c.report.issues {
                    if i.code == "entry.unmapped" {
                        *unmapped.entry(i.message.clone()).or_default() += 1;
                    }
                    *codes
                        .entry((format!("{:?}", i.severity).to_lowercase(), i.code))
                        .or_default() += 1;
                }
                println!(
                    "{name:<60} {n:>5} resources {:>4} report entries {:>7.1} ms",
                    c.report.issues.len(),
                    t.elapsed().as_secs_f64() * 1e3
                );
            }
            Err(e) => {
                failed += 1;
                println!("{name:<60} ERROR {e}");
            }
        }
    }
    println!("\n{ok} converted, {failed} failed\n\nresources:");
    for (t, n) in &types {
        println!("  {t:<28} {n}");
    }
    println!("\nreport entries (severity, code, count):");
    let mut by_count: Vec<_> = codes.iter().collect();
    by_count.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for ((sev, code), n) in by_count {
        println!("  {sev:<8} {code:<46} {n}");
    }
    println!("\nunmapped entries:");
    for (m, n) in &unmapped {
        println!("  {n:>4}  {m}");
    }
}
