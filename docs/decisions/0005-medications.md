# 0005: Medications to MedicationRequest

Status: accepted. Source: IG page `CF-medications`, `CCDA-FHIR MedicationRequest.csv`, ConceptMap `CF-MedicationStatus`.

## Follows the IG
- Status map (active, suspended to on-hold, aborted to stopped, completed, nullified to entered-in-error, cancelled).
  Anything else becomes `unknown` with a warning.
- Intent: moodCode INT is `order`, EVN is `plan`. The EVN choice reads oddly (a recorded, taken medication is a plan?),
  but it is the IG's published rule, and `intent` is required, so we follow it and revisit with the validator and
  Medplum/Microsoft comparison.
- `repeatNumber` is `timing.repeat.count` for INT only; on EVN it has no element and is reported.
- Author is `requester`, with Provenance; first performer is `performer` (R4 allows one).
- Drug vehicle, administrationUnitCode, supply order and dispense are reported, not mapped.
- The drug is a plain `medicationCodeableConcept`; with a manufacturer it becomes a `Medication` resource that holds
  the code and manufacturer, referenced by `medicationReference`.

## Deliberate differences
- `maxDoseQuantity` (numerator over denominator, for example 6 sprays per day) is `maxDosePerPeriod` (a Ratio). The IG
  maps it to `maxDosePerAdministration`, which is a single Quantity and cannot carry the denominator.
- A Free Text Sig is `Dosage.text` (what FHIR defines that element for). An Instruction is `patientInstruction`.
  The IG sends the sig to `patientInstruction` only when there is no Instruction.
- A `precondition` sets `asNeededBoolean` and is reported (`medication.precondition.as-needed-assumed`); the IG says
  not supported. Losing "as needed" is clinically worse than a documented assumption.
- A negated activity sets `doNotPerform` and is reported.
- The `us-core-medicationrequest` profile is claimed only when `authoredOn` and `requester` are present.

## Timing
`effectiveTime` is kept in order. IVL_TS gives `boundsPeriod`; PIVL_TS gives `period`/`periodMax`/`periodUnit`
(UCUM units FHIR allows); EIVL_TS gives `when` and an `offset` in whole minutes. Complex types are reported.
