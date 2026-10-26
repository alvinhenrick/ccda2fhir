# 66. Findings from the HL7 example corpus

Date: 2026-10-26. Status: accepted.

## Context

CI now converts the complete C-CDA documents in `HL7/C-CDA-Examples` (19 of 446 files; the rest
are template fragments, stubs or malformed XML) and validates the output with the HL7 validator
(`scripts/corpus_gate.py`, `scripts/corpus-known-errors.txt`). The first run found about 120 kinds
of error, most of them wrong displays and a few real mapping gaps.

## Decisions

- An id under the NPI OID that is not a real NPI (10 digits, valid check digit) keeps its value
  under `urn:oid:2.16.840.1.113883.4.6`, not the NPI system (`identifier.npi.invalid`).
- `eng`, `ita` and the other ISO 639-2 codes become BCP 47 (`en`, `it`), on Patient communication
  and Composition language.
- Race and ethnicity `UNK` / `ASKU` use the v3 NullFlavor system, as the OMB value set does.
- Codes are trimmed; a NUCC grouping code (`200000000X`) is kept as `text`, not a coding, because
  validators do not carry it (`coding.nucc-grouping.text-only`).
- A Practitioner with no name gets `name` with a data-absent-reason, so US Core references to it
  validate (`practitioner.name.absent`).
- A negated allergy with a coded substance is written as `code.text` ("no known allergy to ...").
  The IG's extension fallback has no `code`, which US Core requires and the validator rejects, and
  a coded substance would read as an allergy to it (`allergy.negated.substance-text`).
- UCUM: `mcg`, `10+3/ul`, `mg/actuat` and similar spellings are normalised.
- A CDA `caption` outside a table is bold text (a bold list item inside a list).
- Display policy stays as in ADR 0027: the table grows from validator findings. `Terminology` now
  has `official_display`, so a `--loinc-table` with `LONG_COMMON_NAME` supplies LOINC displays for
  the whole code system without bundling it.

## Known, accepted corpus errors

Listed with reasons in `scripts/corpus-known-errors.txt`: CPT displays (AMA licence) and retired
codes, source documents that misuse a code system, and vital-sign codes (3141-9, 59408-5) the base
FHIR profiles do not accept. The corpus job fails on any other error.
