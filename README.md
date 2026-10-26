# ccda2fhir

Convert C-CDA documents to FHIR R4, written in Rust. A pure library first; CLI, web service and web UI are
thin layers on top.

**Status: under active development.** Most USCDI v1 and v3 data classes are converted and checked with the HL7
validator in CI; the remaining gaps are listed in [`docs/uscdi.md`](docs/uscdi.md) and
[`docs/coverage.md`](docs/coverage.md). See [`DESIGN.md`](DESIGN.md) for the plan.

## Run it

### Docker (one command)

```
docker compose up --build
curl -X POST -H 'Content-Type: application/xml' \
  --data-binary @your-ccd.xml 'http://localhost:8080/convert' > bundle.json
```

`POST /convert` takes the C-CDA XML as the body and returns a FHIR R4 Bundle (`application/fhir+json`). Add
`?report=true` to get `{ "bundle": ..., "report": [...] }`, or `?profiles=none` to leave out `meta.profile`.
Errors are `OperationOutcome` resources. `GET /healthz` and `GET /readyz` are for orchestrators.

Settings (environment variables): `BIND_ADDR` (default `0.0.0.0:8080` in the image), `MAX_BODY_BYTES`
(10 MiB), `MAX_CONCURRENCY` (64), `REQUEST_TIMEOUT_SECS` (30), `LOINC_TABLE` (optional path to your own LOINC `Loinc.csv`, used to set result categories; also `convert --loinc-table`), `RUST_LOG` (`info`). Request bodies are never
logged. The service has no authentication yet: run it inside your network or behind a gateway that adds it.

### Check your own documents with the HL7 validator

An opt-in test rig starts the service and the official HL7 validator, converts every `.xml` in a folder and
validates the bundles against FHIR R4 and US Core 6.1.0:

```
INPUT_DIR=./my-documents docker compose --profile validate up --build \
  --abort-on-container-exit --exit-code-from validator
docker compose --profile validate down -v      # tear everything down
```

The default input is the repository's synthetic fixtures. A normal `docker compose up` does not start it.

### Command line

```
cargo run --release -p ccda2fhir-server -- convert ccd.xml > bundle.json
cargo run --release -p ccda2fhir-server -- convert *.xml --out-dir out/
cargo run --release -p ccda2fhir-server -- serve --bind 127.0.0.1:8080
```

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
| `ccda2fhir-server` | The `ccda2fhir` command line tool and HTTP service |

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
