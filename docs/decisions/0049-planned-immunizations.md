# 0049: Planned immunizations

## Status
Accepted

## Context
The IG maps only historical immunizations (mood EVN) to `Immunization` and says to use
`MedicationRequest` for planned ones (mood INT). ADR 0019 left them unmapped.

## Decision
A Planned Immunization Activity (4.120), and an Immunization Activity (4.52) whose mood is not
EVN, is mapped by the medication mapper to a `MedicationRequest`, with the intent rule of ADR 0005
(INT is an order). The vaccine code (CVX) becomes `medicationCodeableConcept`. The immunization
mapper no longer reports these entries; the medication mapper reports a mood other than EVN or
INT (`medication.mood.unsupported`).

## Consequences
- `immunization.intent.unsupported` is gone.
- A planned vaccine is not an `ImmunizationRecommendation`: that resource is a forecast, which a
  C-CDA plan of treatment is not.
