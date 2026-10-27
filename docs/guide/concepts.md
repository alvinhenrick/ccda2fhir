# Concepts: what the output means

## The report

A conversion never silently drops or changes data. Every judgment call becomes a **report entry** with a severity, a
stable `code` and a short message that never contains document values.

- **Warning**: data was dropped, defaulted or approximated. Examples: `coding.system.missing` (a code had no code
  system, so none was written) and `identifier.npi.invalid` (an NPI that fails its check digit is kept under its OID
  instead of the NPI system).
- **Info**: worth knowing, nothing lost. Examples: `condition.status.defaulted` (no status in the document, so
  `active` was used as the IG directs), `coding.display.replaced` (the official display name was used and the
  document's own wording kept as `text`) and `entry.unmapped` (a template this version does not convert, with its
  template id).

`status` is `complete` when there are no warnings and `partial` otherwise. Take `partial` as "read the report", not as
"failed": most real documents are partial.

## Nothing is invented

When FHIR requires an element the document does not give, the converter writes a **data-absent-reason**
(`http://hl7.org/fhir/StructureDefinition/data-absent-reason`, or `_field` for primitives) and reports it. It does not
make up a value to satisfy a validator. Missing performers, unknown statuses and similar gaps stay visible.

## Profiles

A resource declares a US Core 6.1.0 profile in `meta.profile` only when everything that profile requires is present.
A resource that cannot meet its profile is still emitted, just without the claim. `?profiles=none` (or
`Profiles::None`) declares nothing.

## Codes and displays

Code systems are written as their FHIR URIs (`http://loinc.org`, `http://snomed.info/sct`) wherever the OID is known,
and as `urn:oid:...` otherwise. When a code is in the converter's checked table and the document's display name
differs from the official one, the official display is used (validators reject wrong displays) and the document's
wording is kept as `text`. A code that is not in the table keeps the document's display; nothing is guessed.

## Negation, planned and historical statements

A negated problem or allergy ("no known allergy to X") keeps its meaning rather than becoming a positive entry. A
planned medication, procedure or encounter (mood INT, RQO, PRP, APT) becomes a request (`MedicationRequest`,
`ServiceRequest`, `Appointment`), not a record of something done.

## Narrative

Section narrative is kept as the `Composition` section text, as XHTML. Resources get a short generated narrative from
their own fields. Treat `text.div` as untrusted markup, as FHIR says: it is escaped by the converter, but sanitise it
if you render it as HTML.

## Ids and links

Resource ids are sequential per type (`condition-1`); `fullUrl` values are UUID v5 derived from the document id.
References inside the bundle (encounter, performer, subject, `addresses`, `reasonReference`, `derivedFrom`) are by
`fullUrl`, so a bundle can be stored as is.

## What is not converted

The element-by-element status is in the [coverage tables](../coverage.md) and the [USCDI status](../uscdi.md).
Known gaps are listed there; templates the converter does not handle are reported as `entry.unmapped` instead of
being ignored.
