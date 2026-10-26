# 0050: reasonReference from an indication

## Status
Accepted

## Context
An Indication (4.19) is mapped to `reasonCode` on the resource it explains. C-CDA lets an
Indication carry the id of the Problem Observation it stands for, so the reason can also be a
link to the Condition.

## Decision
After all resources exist, an Indication whose id matches the identifier of a mapped Condition adds
a `reasonReference` to that Condition on the Encounter, MedicationRequest, Procedure, ServiceRequest
or Immunization made from the parent entry. `reasonCode` is kept, so nothing the document said is
lost. Matching is by identifier, as for encounter links (ADR 0025); an Indication that names no
Condition adds nothing and is not reported, because most Indications are only a code.

## Consequences
- Only Conditions are linked. Reasons that are Observations stay as `reasonCode`.

Observations are linked too since ADR 0062.
