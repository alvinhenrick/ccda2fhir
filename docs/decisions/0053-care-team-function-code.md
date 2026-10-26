# 0053: Care team member function code

## Status
Accepted

## Context
ADR 0016 took a care team member's role only from the performer's code (a specialty). C-CDA also
says what the member does on the team with a `functionCode`.

## Decision
`CareTeam.participant.role` lists, in order and without repeats: the performer's `functionCode`,
the member act's `functionCode`, then the performer's own code. The model reads `functionCode` on
every performer.

## Consequences
- Patients and related persons as members are still not mapped.
