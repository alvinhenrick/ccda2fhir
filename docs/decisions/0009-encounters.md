# 0009: Encounters to Encounter

Status: accepted. Source: IG page "CF-encounters", ConceptMap `CF-EncounterStatus`.

- `class` is a v3 ActCode coding. It comes from an ActCode value in the FHIR class set (AMB, EMER, IMP, ...) in the code or
  its translations, else from a CPT E/M range (99201-99215 AMB, 99221-99223 IMP, 99281-99285 EMER). With neither, class is
  marked with a data-absent reason and reported. 99201-99205 are our addition to the IG's examples (office visits).
- `type` holds the other codes; the ActCode value used for class is removed from it.
- Status: the effective time decides (a point or an end is finished; a start alone is in-progress), unless the statusCode is
  something other than completed/active, then the IG map is used. The map covers the rest and the missing-time case.
- Encounter Diagnosis acts become `encounter-diagnosis` Conditions, mapped with the problem rules, linked both ways.
- `us-core-encounter` is claimed when identifier, class and type are present.
- Not done yet: the header's encompassingEncounter, merging duplicates, Location, participant function, and `encounter`
  links from other resources.
