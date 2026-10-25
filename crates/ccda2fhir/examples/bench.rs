//! Throughput check: converts the given files repeatedly and prints documents per second.
//!
//! `cargo run --release -p ccda2fhir --example bench -- [--rounds N] <files...>`

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::time::Instant;

use ccda2fhir::{convert, Options};

fn main() {
    let mut rounds = 20usize;
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--rounds" {
            rounds = args.next().and_then(|v| v.parse().ok()).unwrap_or(rounds);
        } else {
            files.push(a);
        }
    }
    let docs: Vec<Vec<u8>> = files.iter().filter_map(|f| std::fs::read(f).ok()).collect();
    let bytes: usize = docs.iter().map(Vec::len).sum();
    let options = Options::default();
    let mut ok = 0usize;
    let start = Instant::now();
    for _ in 0..rounds {
        for d in &docs {
            if convert(d, &options).is_ok() {
                ok += 1;
            }
        }
    }
    let secs = start.elapsed().as_secs_f64();
    println!(
        "{ok} conversions of {} documents ({} KB) in {secs:.2}s: {:.0} docs/s, {:.1} MB/s",
        docs.len(),
        bytes / 1024,
        ok as f64 / secs,
        (bytes * rounds) as f64 / secs / 1_048_576.0,
    );
}
