# fhir-out

FHIR R4 output building blocks, kept separate from C-CDA parsing and mapping.

A `Bundle` type with deterministic ids: `fullUrl` values are UUID v5 derived from a seed, so converting the same document twice gives byte-identical output and bundles from different documents never collide.

Part of [ccda2fhir](https://github.com/alvinhenrick/ccda2fhir), an open source C-CDA to FHIR R4 converter. MIT licensed. Use synthetic or de-identified data until you have done your own security and compliance review.
