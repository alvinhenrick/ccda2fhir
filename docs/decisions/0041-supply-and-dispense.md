# 0041. Supply orders and dispenses

Status: accepted

## Context
A Medication Activity can carry a Supply Order (4.17) and Medication Dispense (4.18). They were
only reported as unsupported.

## Decision
- Supply Order maps into `MedicationRequest.dispenseRequest`: `quantity`,
  `numberOfRepeatsAllowed` (from `repeatNumber`, used as written) and `validityPeriod`
  (from `effectiveTime` low/high). R4 allows one dispense request, so a second order is dropped
  and reported as `medication.supply.multiple`.
- Medication Dispense maps to a `MedicationDispense` with `authorizingPrescription` pointing at
  the request, `whenHandedOver`, `quantity` and `performer.actor`. The drug is the dispense's own
  product when present, otherwise the request's drug.
- Status follows the ActStatus map (`active` to `in-progress`, `aborted` to `stopped`, ...);
  anything else is `unknown` and reported as `dispense.status.unmapped`.
- No US Core MedicationDispense profile is claimed: it needs elements C-CDA does not carry.
- The model reads `product` as well as `consumable`, since supply acts use `product`.
