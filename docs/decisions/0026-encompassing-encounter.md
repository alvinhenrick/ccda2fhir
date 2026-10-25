# 0026: The header's encompassing encounter

Status: accepted.

- `componentOf/encompassingEncounter` is parsed as an Encounter statement: id, code, time, discharge disposition, the
  `encounterParticipant`s as performers and the `healthCareFacility` as a location.
- It maps to an `Encounter` exactly like an Encounter Activity (ADR 0005 rules for class, type, status and period), and
  `Composition.encounter` points at it.
- If the body already has an Encounter with the same identifier, that Encounter is the encompassing encounter: nothing is added
  and the Composition points at it. An encompassing encounter with no identifier is always added.
- Entries are not linked to the encompassing encounter implicitly. C-CDA says an entry happened during it only when the entry
  says so (ADR 0025).
