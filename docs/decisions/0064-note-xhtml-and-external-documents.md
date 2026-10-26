# 64. Note content as XHTML, external document references

Date: 2026-10-25. Status: accepted.

## Context

The IG page CF-notes says a note's `text/reference` is converted with the narrative guidance and
uses `application/xhtml+xml`, and that `reference/externalDocument/id` maps to
`relatesTo.target.identifier`. Until now notes were flattened to plain text and external
document references were dropped.

## Decision

- The model renders each narrative element that has an `ID` and structure inside (lists, tables,
  styled spans) as XHTML with the same whitelist renderer as section text (ADR 0052), and the
  statement exposes it as `text_xhtml`.
- A note whose narrative has markup gets `application/xhtml+xml`; plain-text narrative stays
  `text/plain`; a note that states another media type keeps it.
- The IG maps each `externalDocument` id to `relatesTo.target.identifier`. R4 requires
  `relatesTo.code`, the source gives no relationship, and the HL7 validator rejects a missing
  code (a data-absent-reason on it does not help). We deviate: the identifier goes to
  `context.related` (an identifier-only reference), and `note.externaldocument.related` is
  reported. No relationship is claimed.
- A source `DocumentReference` for the converted document is not created: no IG page maps it.
- Report narrative types (imaging, lab, pathology narrative sections) stay text: the IG has no
  separate mapping for them.

## Consequences

Tables and lists in notes survive conversion. No relationship is invented for linked documents; if a later IG supplies a code, move the id to `relatesTo`.
