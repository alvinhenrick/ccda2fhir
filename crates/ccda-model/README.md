# ccda-model

A typed, version-neutral model of a C-CDA document and a safe parser for it.

The parser rejects any `DOCTYPE`, so entity-expansion attacks do not apply, and enforces limits on input size, node count and nesting depth (`Limits`). Error messages never contain document content.

```rust
let document = ccda_model::parse(xml_bytes)?;
```

You usually do not use this crate directly; `ccda2fhir` does it for you.

Part of [ccda2fhir](https://github.com/alvinhenrick/ccda2fhir), an open source C-CDA to FHIR R4 converter. MIT licensed. Use synthetic or de-identified data until you have done your own security and compliance review.
