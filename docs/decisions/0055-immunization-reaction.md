# 0055: Immunization reaction

## Status
Accepted

## Context
The IG recommends mapping the Reaction Observation (4.9) of an immunization to
`Immunization.reaction.detail`, an Observation whose `valueCodeableConcept` is the reaction.

## Decision
- Each Reaction Observation with a coded value (or a code other than the generic ASSERTION)
  becomes an `Observation` (status final, subject the patient, `effectiveDateTime` and
  `valueCodeableConcept` from the source) and an entry in `Immunization.reaction` with `detail`
  and `date`. `Observation.code` is required; when the source has none, it carries a
  data-absent-reason. A reaction with neither code nor value is reported
  (`immunization.reaction.empty`).
- A dose given as a range is still omitted and reported. `Immunization` has one `doseQuantity`,
  the IG has no range mapping, and picking one end would invent a dose.

## Consequences
- `immunization.reaction.unsupported` is gone.
