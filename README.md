# ccda2fhir

Convert C-CDA documents to FHIR R4, written in Rust. A pure library first; CLI, web service and web UI are
thin layers on top.

**Status: 0.1.0** (see [`CHANGELOG.md`](CHANGELOG.md)). Most USCDI v1 and v3 data classes are converted and checked with the HL7
validator in CI; the remaining gaps are listed in [`docs/uscdi.md`](docs/uscdi.md) and
[`docs/coverage.md`](docs/coverage.md); [`docs/benchmarks.md`](docs/benchmarks.md) has throughput numbers. See [`DESIGN.md`](DESIGN.md) for the plan.

## Run it

### Docker (one command)

```
docker compose up --build
curl -X POST -H 'Content-Type: application/xml' \
  --data-binary @your-ccd.xml 'http://localhost:8080/convert' > bundle.json
```

`POST /convert` takes the C-CDA XML as the body and returns a FHIR R4 Bundle (`application/fhir+json`). Add
`?report=true` to get `{ "status", "issues", "bundle", "report" }`, or `?profiles=none` to leave out `meta.profile`.

`status` is `complete` when nothing was dropped, defaulted or approximated, and `partial` when the
report holds at least one warning. The same value is in the `X-Ccda2fhir-Status` header.

The document can also arrive as JSON, for callers that cannot send raw XML:
`{ "document": "<base64 C-CDA>" }`, or a FHIR `DocumentReference` whose XML attachment has inline
base64 `data` (`Content-Type: application/json` or `application/fhir+json`).
Errors are `OperationOutcome` resources. `GET /healthz` and `GET /readyz` are for orchestrators.

Open `http://localhost:8080/` for a built-in page: drop or paste a C-CDA file, pick the profiles, and see the resources, the
conversion report and the bundle JSON side by side. It is plain HTML, CSS and JS served by the service itself (no build step, no
external requests, a strict Content-Security-Policy), and the document goes only to `POST /convert`. If authentication is on,
paste a bearer token into the page; it is kept in memory only. Turn the page off with `DISABLE_UI=1` (or `serve --no-ui`).

Settings (environment variables): `BIND_ADDR` (default `0.0.0.0:8080` in the image), `MAX_BODY_BYTES`
(10 MiB), `MAX_CONCURRENCY` (64), `REQUEST_TIMEOUT_SECS` (30), `LOINC_TABLE` (optional path to your own LOINC `Loinc.csv`, used to set result categories; also `convert --loinc-table`), `RUST_LOG` (`info`). Request bodies are never
logged.

#### Authentication (optional)

Off by default. Set a key source and `/convert` then needs `Authorization: Bearer <JWT>` from your identity
provider (Auth0, Okta, Keycloak, Entra, Cognito, anything that publishes a JWKS). `/healthz` and `/readyz` stay open.

| Variable | Meaning |
| --- | --- |
| `AUTH_JWKS_URL` | The provider's JWKS (`https://`; plain `http://` only for localhost). Turns authentication on. |
| `AUTH_JWKS` | The JWKS JSON inline, instead of a URL (air-gapped or fixed keys). |
| `AUTH_ISSUER`, `AUTH_AUDIENCE` | Required with either source: the `iss` and `aud` a token must carry. |
| `AUTH_REQUIRED_SCOPE` | Optional scope the token must grant (`scope` string or `scp`). Missing scope is a 403. |
| `AUTH_ALGORITHMS` | `RS256,ES256` (default). Nothing else is supported; `none` and `HS*` cannot be enabled. |
| `AUTH_LEEWAY_SECS`, `AUTH_JWKS_MIN_REFRESH_SECS`, `AUTH_JWKS_TTL_SECS` | Clock skew (60), shortest gap between key fetches (300), key lifetime (3600). |

A bad or missing token is a 401 with a `WWW-Authenticate` challenge; the token is never logged or echoed. Starting
with `AUTH_ISSUER` or `AUTH_AUDIENCE` but no key set is an error, so a typo cannot silently leave the service open.
See [ADR 0067](docs/decisions/0067-jwt-authentication.md).

### Check your own documents with the HL7 validator

An opt-in test rig starts the service and the official HL7 validator, converts every `.xml` in a folder and
validates the bundles against FHIR R4 and US Core 6.1.0:

```
INPUT_DIR=./my-documents docker compose --profile validate up --build \
  --abort-on-container-exit --exit-code-from validator
docker compose --profile validate down -v      # tear everything down
```

The default input is the repository's synthetic fixtures. A normal `docker compose up` does not start it.

### In the browser or Node (WebAssembly)

The same converter compiles to WebAssembly, so a document can be converted without leaving the machine.

```
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --locked --version <the version in Cargo.lock>
scripts/build-wasm.sh web        # or: nodejs
```

```js
import init, { convert_ccda } from "./ccda2fhir_wasm.js";
await init();
const result = JSON.parse(convert_ccda(xmlText, "us-core"));   // { status, issues, bundle, report }
```

CI checks that the WebAssembly build produces bundles identical to the native goldens. See ADR 0069.

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

## Documentation

The documentation site (API reference, coverage tables, design decisions, lessons and a live in-browser demo) is built by
`scripts/build-site.sh` and published to GitHub Pages from `main` (`https://alvinhenrick.github.io/ccda2fhir/`). To preview it
locally: `scripts/build-wasm.sh web && scripts/build-site.sh`, then serve `target/site`. Pass `SKIP_DEMO=1` to leave the demo out.

## Workspace

| Crate | Role |
|---|---|
| `ccda-model` | Typed, version-neutral C-CDA model and a safe parser (size and node limits, no DTD) |
| `ccda-terminology` | OID to FHIR system URI mapping and a checked table of official displays, behind a `Terminology` trait |
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

The library parses untrusted XML safely by default. The service can verify JWT bearer tokens (above); TLS and
auditing belong to your gateway or the next layer; see [`SECURITY.md`](SECURITY.md). Do not use with real patient
data in an unauthenticated deployment.

## License

[MIT](LICENSE)
