# ccda2fhir — Design and Plan

A C-CDA → FHIR converter written in Rust, built as a reusable library first, with a CLI, web service and
web UI as thin layers on top. Goal: exhaustive USCDI v1 and v3 coverage, verified by tests and the HL7 validator.

## 1. Targets (pinned, extensible)

| Dimension | v1 target | v3 target | Status |
|---|---|---|---|
| FHIR | R4 (4.0.1) | R4 (4.0.1) | decided |
| US Core | 3.1.1 | 6.1.0 | **verify pairing against US Core pages before locking** |
| USCDI | v1 (July 2020 errata) | v3.1 (June 2025) | decided |
| C-CDA | R2.1 | R2.1 | R1.1 deferred |

Version differences are *data* (rule sets keyed by target), not scattered `if` statements.

## 2. Definition of "exhaustive"

For every USCDI data element (v1 and v3), exactly one of:
1. A mapping rule exists, a golden test covers it, and the output validates against the target US Core profile.
2. It is marked `unmappable-from-ccda` with a recorded reason, and a warning is emitted when encountered.

A generated coverage matrix (`docs/coverage.md`) is the source of truth for "done".

## 3. Architecture

```
C-CDA XML
  → parse + limits         (ccda-model)    XML → typed, version-neutral model
  → normalize              (ccda-model)    nullFlavor, dates, OID→URI, codes
  → map                    (ccda-map)      per-template rules + handlers → FHIR resources
  → resolve + post-process (ccda-map)      ids, dedup, references, Provenance, narrative
  → emit                   (fhir-out)      Bundle (R4)
  → report                 (ccda2fhir)     warnings, unmapped items, stats
```

### Crates
- `ccda-model` — typed intermediate model and parser (safe by default: size/depth limits, no DTD/entities).
- `ccda-terminology` — `Terminology` trait, OID↔URI table, code translation (offline by default).
- `ccda-map` — rule engine, template handlers, id/reference context.
- `fhir-out` — FHIR R4 output types and Bundle builder (kept isolated so other FHIR versions can be added).
- `ccda2fhir` — public facade: `convert(xml, &Options) -> Conversion { bundle, report }`.
- `ccda2fhir-cli`, `ccda2fhir-server` (axum), `bindings/` (WASM, Python, Node) — later phases.

### Principles
- The library is pure: no I/O, no global state. Wrappers add I/O.
- Lossless by default: anything unmapped goes to an extension or Provenance and is listed in the report.
- Deterministic ids: same input → same output.
- Errors about *data* are values (`Report`), not panics.
- No PHI in logs or error messages. Log template ids, counts, error codes only.

## 4. Security (designed in, implemented later)

Now (cheap, hard to retrofit): XML limits, PHI-free logging, stateless by default, localhost bind by default.

Seams (traits with permissive defaults, implemented by orgs or us later):
`Authenticator`, `Authorizer`, `AuditSink`. Later: OIDC/JWT validation, TLS (rustls), RBAC, rate limits, audit export.

## 5. Phases

| Phase | Deliverable |
|---|---|
| 0 | Workspace, design, coverage matrix generator, test harness, validator in CI |
| 1 | Typed model + header (patient, author, custodian), terminology, ids/references, Provenance, narrative |
| 2 | USCDI v1 classes vs US Core 3.1.1, each validated before moving on |
| 3 | USCDI v3 additions vs US Core 6.1.0 (encounters, insurance, health status, SDOH, clinical tests, imaging, richer meds/labs/demographics) |
| 4 | Hardening on messy real-world samples, fuzzing, performance |
| 5 | CLI, REST service, web UI, WASM build |
| 6 | Security layer, deployment artifacts, docs, independent review |

Effort (judgment, to be replaced by real numbers from the coverage matrix): roughly 6–9 months for one engineer to
reach validated v1+v3 coverage; the long tail of non-conformant real documents is the largest share.

## 6. Learning track

Each phase ships with a short lesson (`LESSONS/NN-*.md`) teaching only the Rust concepts that phase uses, tied to
real code in the repo.

## 7. Open questions
- Confirm US Core ↔ USCDI version pairing.
- Choose FHIR type strategy: generated from StructureDefinitions vs an existing R4 crate vs `serde_json::Value`.
- Source of real-world (de-identified) sample documents.
