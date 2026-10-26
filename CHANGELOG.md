# Changelog

## 0.1.0

First release. A C-CDA to FHIR R4 converter in Rust, library first, with a command line tool and an
HTTP service.

### Conversion

- C-CDA R2.1 and 3.0 documents to a FHIR R4 (4.0.1) bundle claiming US Core 6.1.0 profiles only where the
  required elements exist. USCDI v1 and v3 data classes are covered; see `docs/uscdi.md` and `docs/coverage.md`
  for every element and the few known gaps.
- Patient and demographics, problems, health concerns, allergies, medications (and planned medications,
  supply and dispense), immunizations, vital signs, results and specimens, procedures, encounters (with links
  and reasons), social history and SDOH, family history, devices and UDI, goals, care plans, care teams,
  coverage, service requests and referrals, assessments, notes and unstructured documents, imaging studies,
  advance directives, Composition with narrative, and Provenance.
- Never invents values: missing required elements carry a data-absent-reason. Anything lossy is listed in a
  report with a stable, document-free code. Output is deterministic.
- Official-display policy for well-known codes (ADR 0027), OID to URI mapping, optional LOINC class table.

### Quality

- Golden tests on synthetic fixtures; the HL7 validator (US Core 6.1.0) runs in CI on the fixtures and on the
  19 complete documents of the HL7 C-CDA examples (a handful of documented source-data errors remain).
- Fuzzing of the parser and converter, `cargo deny` for licences and advisories, a Docker image with a
  validator test rig.

### Service and CLI

- `ccda2fhir convert`, `serve` and `healthcheck`. `POST /convert` takes XML, base64 JSON or a
  DocumentReference, returns the bundle with a per-document status, and `?report=true` adds the report.
- Optional JWT bearer authentication (RS256 and ES256, JWKS by URL or inline, issuer, audience, scope).
  See ADR 0067.

### Not in 0.1

Web UI, WASM build, TLS termination, per-client rate limits and audit export. ONC certification is not a goal.
