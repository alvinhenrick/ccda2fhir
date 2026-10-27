# ccda-terminology

Code system helpers for C-CDA to FHIR conversion: OID to FHIR system URI mapping and a checked table of official display names.

The `Terminology` trait is what the mappers ask; `BuiltinTerminology` answers from tables in the crate and `LoincTable` can add LOINC classes and names from a `Loinc.csv` you download yourself (it is not bundled).

When a code is in the table and a document's display name differs, the official display is used; unknown codes keep the document's own display. Nothing is guessed.

Part of [ccda2fhir](https://github.com/alvinhenrick/ccda2fhir), an open source C-CDA to FHIR R4 converter. MIT licensed. Use synthetic or de-identified data until you have done your own security and compliance review.
