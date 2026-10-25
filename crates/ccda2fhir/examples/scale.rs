//! Scaling check: repeat a fixture's body N times and time the conversion.
//!
//! Usage: `cargo run --release -p ccda2fhir --example scale -- <fixture.xml> <n> [<n> ...]`
//! Time per input byte should stay flat as N grows; a rising figure means super-linear work.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::time::Instant;

use ccda2fhir::{convert, Options};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("fixture path");
    let xml = std::fs::read_to_string(path).unwrap();
    let start = xml.find("<structuredBody>").unwrap() + "<structuredBody>".len();
    let end = xml.find("</structuredBody>").unwrap();
    for n in args.map(|a| a.parse::<usize>().unwrap()) {
        let doc = format!(
            "{}{}{}",
            &xml[..start],
            xml[start..end].repeat(n),
            &xml[end..]
        );
        let t = Instant::now();
        let c = convert(doc.as_bytes(), &Options::default()).unwrap();
        let dt = t.elapsed();
        println!(
            "n={n:<5} {:>8} KiB  {:>8.1} ms  {:>6.2} us/KiB  {} resources",
            doc.len() / 1024,
            dt.as_secs_f64() * 1e3,
            dt.as_secs_f64() * 1e6 / (doc.len() as f64 / 1024.0),
            c.bundle.len()
        );
    }
}
