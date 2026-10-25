# ccda2fhir

Convert C-CDA documents to FHIR R4, written in Rust. A pure library first; CLI, web service and web UI are
thin layers on top.

**Status: early development.** Only the document header's Patient (name, gender, birth date) is converted today.
The goal is exhaustive USCDI v1 and v3 coverage, with every claim backed by a test and by the HL7 validator.
See [`DESIGN.md`](DESIGN.md) for the plan.

## Quick start

```rust
use ccda2fhir::{convert, Options};

let conversion = convert(&std::fs::read("ccd.xml")?, &Options::default())?;
println!("{}", conversion.bundle.to_json());
for issue in &conversion.report.issues {
    eprintln!("{:?} {}: {}", issue.severity, issue.code, issue.message);
}
```

Conversion never silently drops data: anything lossy or unexpected appears in `conversion.report`.
Error and report messages never contain document content, so they are safe to log.

## Workspace

| Crate | Role |
|---|---|
| `ccda-model` | Typed, version-neutral C-CDA model and a safe parser (size and node limits, no DTD) |
| `ccda-terminology` | OID to FHIR system URI mapping behind a `Terminology` trait |
| `ccda-map` | Maps the model to FHIR resources and records issues in a `Report` |
| `fhir-out` | FHIR R4 output building blocks (Bundle) |
| `ccda2fhir` | Public facade: `convert()` |

## Development

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

New to Rust? [`LESSONS/`](LESSONS) walks through the code step by step.

## Security

The library parses untrusted XML safely by default. Authentication, TLS and auditing belong to the service layer
and are designed as pluggable seams; see [`SECURITY.md`](SECURITY.md). Do not use with real patient data in an
unauthenticated deployment.

## License

[MIT](LICENSE)
