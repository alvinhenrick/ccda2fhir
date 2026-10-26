# 65. Service: transformation status and JSON input

Date: 2026-10-26. Status: accepted.

## Decision

- `POST /convert` reports a per-document `status`: `complete` (no warnings) or `partial` (at
  least one warning: data was dropped, defaulted or approximated). With `?report=true` the
  response is `{ status, issues: { total, warnings }, bundle, report }`; the header
  `X-Ccda2fhir-Status` always carries it. Info-level notes do not make a document partial.
- The request may be JSON: `{ "document": "<base64>" }` or a `DocumentReference` whose first XML
  attachment has inline `data`. Invalid JSON, base64 or a missing attachment is a 400 whose
  message never echoes the request. A `url` attachment is not fetched (no outbound requests
  from the service). Other media types stay 415.
