# 0054: Specimen condition, and what Location cannot have

## Status
Accepted

## Context
ADR 0018 listed Specimen condition and several Location and Encounter location elements as not
mapped.

## Decision
- The Specimen Condition Observation (4.421), nested in the Specimen Collection Procedure, becomes
  `Specimen.condition` from its coded value (HL7 table 0493, OID 2.16.840.1.113883.18.317, mapped
  to the v2-0493 URI). The IG gives `/value` as the source.
- `Location.status`, `Location.managingOrganization`, `Encounter.location.status` and `period`, and
  `Encounter.serviceProvider` from the location have no source in the C-CDA: the Service Delivery
  Location role carries an id, a type, a name, an address and a telecom only. They stay unmapped on
  purpose, and are not guessed (for example, `status` is not set to `active`).

## Consequences
- These items are removed from the list of gaps.
