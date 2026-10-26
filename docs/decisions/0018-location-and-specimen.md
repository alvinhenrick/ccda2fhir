# 0018: Locations and Specimens

Status: accepted. Specimen rules come from the IG page "CF-results"; Location follows C-CDA Service Delivery Location (4.32) and the
US Core Location profile.

## Location

- A `participant typeCode="LOC"` on an encounter or procedure becomes a `Location` (shared by identifier, else name) and is
  referenced from `Encounter.location.location` or `Procedure.location`.
- `name` is the playing entity name, `type` the role code (HSLOC maps to the CDC URI), plus identifier, address and telecom.
  US Core requires a name, so a location without one is not mapped and is reported (`location.name.missing`).
- Not mapped yet: `Encounter.location.status`/`period`, `serviceProvider` from the location's organization, Location
  `managingOrganization`.

## Specimen

- A `specimen/specimenRole` becomes a `Specimen`: `identifier` from the role id, `type` from the playing entity code (name becomes
  `type.text`, or a `note` when a code exists), `note` from `desc`, `collection.quantity` from `quantity`, and `subject` the patient.
- The organizer's Specimen Collection Procedure (SNOMED 17636008) gives `collection.bodySite` and `collectedDateTime`.
- A specimen on a Result Organizer is attached to the DiagnosticReport (`specimen`) and to each child Observation; a specimen on an
  observation is attached to that observation only. FHIR Observation allows one specimen, so a second one is dropped from the
  Observation (it stays on the report).
- A specimen with no code, name or id is not mapped and is reported (`specimen.empty`).
- Specimen Condition Observation (`condition`) is not mapped yet.

## Update (docs refresh)

Specimen condition was done in ADR 0054. `Encounter.location.status`/`period`, `serviceProvider` and Location `managingOrganization` have no source in the document (ADR 0054).
