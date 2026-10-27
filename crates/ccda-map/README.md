# ccda-map

Maps the typed C-CDA model to FHIR R4 resources and reports anything lossy.

One mapper per area (problems, allergies, medications, vital signs, results, procedures, encounters, social history, care plans, care teams and more), plus the linking passes that connect them. Every judgment call is recorded in a `Report` with a stable code and no document values.

You usually want `ccda2fhir`, which runs the whole pipeline; use this crate to run individual mappers.

Part of [ccda2fhir](https://github.com/alvinhenrick/ccda2fhir), an open source C-CDA to FHIR R4 converter. MIT licensed. Use synthetic or de-identified data until you have done your own security and compliance review.
