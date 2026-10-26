# 0053: Care team member function code

## Status
Accepted

## Context
ADR 0016 took a care team member's role only from the performer's code (a specialty). C-CDA also
says what the member does on the team with a `functionCode`.

## Decision
`CareTeam.participant.role` is the performer's `functionCode`, else the member act's
`functionCode`, else the performer's own code (a specialty). US Core allows one role, so the
specialty is dropped when a function code exists (the validator rejects two). The model reads
`functionCode` on every performer.

## Consequences
- Patients and related persons as members are still not mapped.
