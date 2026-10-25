# 0028: Family history

Status: accepted. The IG has no family history page; this follows FHIR R4 `FamilyMemberHistory` and the C-CDA templates.

- A Family History Organizer (4.45) is one relative and becomes one `FamilyMemberHistory`: `relationship` from
  `subject/relatedSubject/code` (HL7 RoleCode, `FAMMEMB` for a generic "family member"), `name`, `sex`, `bornDate`, and
  `deceasedDate` or `deceasedBoolean` from the relative's `sdtc:` death elements. `status` is `completed`, `partial` for
  other statuses and `entered-in-error` for nullified.
- Each Family History Observation (4.46) becomes a `condition`. A nested Age Observation (4.31) becomes `onsetAge`, and a nested
  Family History Death Observation (4.47) becomes `contributedToDeath`.
- **Negation.** R4 has no way to say a relative did not have a condition. A negated observation ("father did not have asthma")
  is kept as a `note` ("Does not have: ...") and reported (`familyhistory.condition.negated`); it is never turned into a
  positive condition.
- The relative's own id (`sdtc:id`) is used as the identifier only if the organizer has none; otherwise it is reported
  (`familyhistory.subject-id.not-mapped`) because the resource has nowhere to carry it.
- No US Core profile is claimed (US Core has none for family history).
