# 0032. Unstructured documents

Status: accepted

## Context

A C-CDA can carry `nonXMLBody` (an embedded PDF, text or image, or a pointer to a file) instead of sections.

## Decision

- The body becomes one `DocumentReference` (`masterIdentifier` from the document id, `type` from the document code, authors, title as `description`).
- `representation="B64"` data is kept as `attachment.data` after a base64 check; invalid data is dropped with `unstructured.content.invalid-base64`.
- Plain text is base64 encoded, with `text/plain` when no media type is given.
- A `reference` becomes `attachment.url` and is reported as `unstructured.reference.external`, since the file is not in the input. The media type is taken from the file extension only when the document gives none and the extension is known.
- A body with neither data nor a reference is reported as `unstructured.content.missing`.
- The US Core profile is claimed only when `data` and `type` exist (existing rule).

## Consequences

Golden `unstructured` passes the HL7 validator. Narrative generation (`Resource.text`) is separate work.
