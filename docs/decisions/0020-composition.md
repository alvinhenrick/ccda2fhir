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

## Update: always built, and `event`

- A document with no usable author or date still gets a Composition. The missing element is marked with the data-absent-reason
  extension (`author` as an extension-only reference, `_date`), and the report says so (`composition.author.missing`,
  `composition.date.missing`). We do not invent an author or the current time. Before this, 19 of 29 real-world sample documents
  got no Composition, and lost their header and section structure.
- An author that is software becomes a `Device`, one that is only an organization an `Organization`.
- `documentationOf/serviceEvent` becomes `Composition.event`: `code`, `period` from its effective time, and its performers as
  Practitioners in `detail` (`event` has no performer element; the performer's function code is not mapped yet).
