# 0012: Notes to DocumentReference

Status: accepted. Source: IG page "CF-notes".

- A Note Activity (act 4.202) is a `DocumentReference` with category clinical-note and status current. `type` is the LOINC
  translation when there is one, else 34109-9.
- Content: embedded base64 with a media type is passed through (checked to be base64; otherwise dropped with a warning).
  A narrative reference becomes `text/plain` base64, because we keep narrative as plain text. XHTML is a later option.
- `date` is an instant, so it is the earliest author time only when that has a time and offset; otherwise omitted, reported.
- `us-core-documentreference` is claimed when type and content data are present.

## Update (docs refresh)

Still open: narrative references as XHTML (they are plain text today).
