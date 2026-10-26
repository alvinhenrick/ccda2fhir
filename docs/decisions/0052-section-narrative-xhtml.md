# 0052: Section narrative as XHTML

## Status
Accepted

## Context
The Composition section `text` held the section narrative flattened to one paragraph, so tables
and lists (medication lists, lab tables) lost their structure.

## Decision
The section `text` is rendered as a FHIR XHTML `div` by copying only the elements the CDA narrative
and FHIR's XHTML subset share: paragraph, content (as `span`), br, list (`ul`/`ol`), item, table,
thead, tbody, tfoot, tr, th, td (with `colspan`/`rowspan`), caption, sup, sub and links.
- Every other attribute is dropped, including `styleCode` and `ID`.
- Text is escaped. A link keeps its `href` only for http, https and mailto.
- `renderMultiMedia` is dropped (it needs binary content); footnotes and unknown tags keep their
  text only; nesting deeper than 24 levels is reduced to text.
- Source indentation is collapsed.
- The status is `additional`: it is the author's own text, not generated from the entries. A
  section with no structure to keep still gets the generated paragraph.

## Consequences
- Nothing from the narrative can inject script, style or images into the output.
- Notes (DocumentReference) still use plain text from a narrative reference.
- Resource narratives are still generated (ADR 0033).
