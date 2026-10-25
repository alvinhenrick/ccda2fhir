# 0003: Generic clinical statements, deterministic ids, exact decimals

Status: accepted. Items marked "verify" need the HL7 validator.

## Context
Every clinical domain in C-CDA is a nest of the same few elements (`act`, `observation`,
`substanceAdministration`, `supply`, `procedure`, `encounter`, `organizer`) distinguished by
`templateId` and `code`. The IG maps by those, not by element name.

## Decisions
- **The model is generic.** `Statement` carries `kind`, `templateId`s, `code`, `effectiveTime`,
  typed `value`s and nested `related` statements. Mappers decide meaning from templates and codes.
  Domain-only elements (for example `consumable` on a medication) are added to the model when that
  domain is implemented.
- **Narrative references resolve in the parser.** `originalText/reference` and `text/reference`
  look up the section's `ID` attributes, so a mapper sees the text without knowing about narrative.
  An id that is not found keeps the reference and yields no text.
- **Bounded nesting.** Statements and sections nest at most 32 deep; beyond that parsing fails
  with `ParseError::TooDeep` rather than risking stack exhaustion or silent truncation.
- **Ids are `<type>-<n>` in document order** (`patient-1`, `condition-1`, ...). Deterministic for
  one input; not stable across edits to the document. A content-derived scheme can replace it
  behind `Context::next_id`.
- **Decimals keep their written precision** (`5.40` stays `5.40`) via `serde_json`'s
  `arbitrary_precision`. Exponent forms are rejected (not valid FHIR decimals).
- **Selection by template, category by section code.** Condition mapping starts at Problem Concern
  Acts (`2.16.840.1.113883.10.20.22.4.3`) and Problem Observations (`...4.4`), both confirmed in the
  IG example. The category comes from the section's LOINC code per `CF-ProblemCategory`.
- **IG gaps we filled** (each reported, never silent): an end date forces `clinicalStatus` to
  `inactive` when the source said active (R4 invariant con-4); no status anywhere defaults to
  `active` as the IG's CSV directs; `negationInd` becomes `verificationStatus = refuted`.

## Not yet done (tracked in docs/coverage.md)
Authors to `recorder`/`recordedDate`/Provenance, Comment Activity to `note`, supporting
observations to `evidence`, `meta.profile`, Bundle `fullUrl`s, and validator confirmation.

## Update (docs refresh)

Done since this was written: authors and Provenance (ADR 0004), Comment Activity to `note` (ADR 0034), `meta.profile` (ADR 0021), Bundle `fullUrl`s, and validator confirmation in CI. Supporting observations (SPRT) to `evidence` are still reported, not mapped.
