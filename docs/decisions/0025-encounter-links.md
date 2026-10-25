# 0025: Encounter links

Status: accepted.

- An entry names its encounter with a nested Encounter Activity (4.49) that carries the encounter's id. After all mappers have
  run, one pass (`encounter_link.rs`) resolves that id against the Encounters made from the document and sets `encounter` on the
  resource made from the entry.
- Matching is by identifier (`system|value`), the same way health concerns find their Conditions (ADR 0022). An entry with no id
  cannot be linked, because nothing ties it to its resource.
- Linked types are those with an R4 `encounter` element: Condition, Observation, Procedure, DiagnosticReport, MedicationRequest,
  Immunization, ServiceRequest, CarePlan. An existing `encounter` (from an Encounter Diagnosis) is never replaced.
- An encounter id that is not in the document gets the info issue `encounter.link.unresolved`. We do not invent an Encounter from
  an id alone.
- Not done: entries nested inside an Encounter Activity are not linked implicitly, and the header's `encompassingEncounter` is
  still open (Composition.encounter).
