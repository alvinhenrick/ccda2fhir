//! Convert one C-CDA file to a FHIR bundle file: `convert_file <input.xml> <output.json>`.
//! Used by `scripts/validate.sh`. The report goes to stderr as codes only (no document values).

#![allow(clippy::expect_used, clippy::print_stderr)]

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        eprintln!("usage: convert_file <input.xml> <output.json>");
        std::process::exit(2);
    };
    let xml = std::fs::read(&input).expect("cannot read input file");
    let conversion =
        ccda2fhir::convert(&xml, &ccda2fhir::Options::default()).expect("conversion failed");
    for issue in &conversion.report.issues {
        eprintln!("{:?} {}", issue.severity, issue.code);
    }
    let json = serde_json::to_string_pretty(&conversion.bundle.to_json()).expect("serialize");
    std::fs::write(&output, json).expect("cannot write output file");
}
