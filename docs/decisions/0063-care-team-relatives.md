# 63. Care team members who are relatives

Date: 2026-10-25. Status: accepted.

## Context

C-CDA 3.0 Care Team Member Act gives every member as `performer/assignedEntity`. A relative is
marked with `assignedPerson/sdtc:asPatientRelationship` (classCode PRS, a required `sdtc:code`
from RoleCode). The spec has no pattern for the patient as a member.

## Decision

- The model reads the sdtc relationship code into `Author.patient_relationship`.
- A member with that code and a name becomes a `RelatedPerson` (relationship = the code,
  name, identifiers, telecom and address from the entity). `participant.member` references it.
- A relative without a name falls back to the Practitioner path, which reports the member as
  unidentified when it has no id either.
- The patient as a team member is not mapped: C-CDA has no pattern for it, so nothing is guessed.

## Consequences

Family caregivers are no longer written as Practitioners. US Core CareTeam allows
`RelatedPerson` as a member.
