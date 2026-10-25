# 0020: The document to Composition

Status: accepted. The C-CDA on FHIR document profiles are outdated, so this follows FHIR R4 Composition directly.

- The bundle stays a `collection`. A `Composition` is added right after the Patient, so a caller that wants a `document`
  bundle only has to change the bundle type.
- Required elements: `author` (document authors that are people, as shared Practitioners) and `date` (document effectiveTime). If
  either is missing, no Composition is built and that is reported (`composition.author.missing`, `composition.date.missing`).
- `status` is `final` (assumed and reported), `type` the document code, `title` the document title (else the type display, else
  "Clinical document", reported), `identifier` the document id, `subject` the patient, `confidentiality` the v3 code when it
  is one of U, L, M, N, R, V (otherwise reported), `language` the languageCode, `custodian` the custodian organization.
- Each section becomes a `section` with title, code, narrative (`text`, escaped) and `entry` references. A section lists the
  mapped resources whose identifier equals an id of a statement in that section (including nested statements). Supporting
  resources (Practitioner, Location, Specimen...) are never listed. A section with no narrative, entries or subsections is left
  out and reported (`composition.section.empty`), because FHIR requires one of them.
- A section marked with a nullFlavor and no content gets `emptyReason` `unavailable`.
- Resources with no identifier cannot be matched to a section; they are in the bundle but not listed. (A fuller fix is for
  each mapper to record its source section; tracked in the coverage matrix.)
- Not mapped yet: `attester` (legal authenticator), `encounter` (encompassing encounter), `event`, `relatesTo`, version and
  set id.
