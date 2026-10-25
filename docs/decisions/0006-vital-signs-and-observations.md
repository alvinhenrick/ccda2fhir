# 0006: Vital signs and shared Observation rules

Status: accepted. Source: IG pages "CF-vitals" and "CF-results", ConceptMap `CF-ResultStatus`.

## Shared (`observation.rs`, reused by Results and later domains)
- Status: ActStatus to observation-status (completed to final, active/held/suspended to registered, aborted/cancelled to
  cancelled). `nullified` is not in the IG map; we use `entered-in-error`. Anything else is `unknown` with a warning.
- Values by `xsi:type`: PQ to Quantity, CD to CodeableConcept, INT, REAL (Quantity with no unit), ST, BL, TS, IVL_PQ to
  Range. A `nullFlavor` value becomes `dataAbsentReason`. Other types are reported.
- Reference ranges: only "normal" ones (interpretation N or none) are kept, as the IG says; others are reported.
- Performer is the latest author, with Provenance. There is no authored-on element, so the author time only feeds Provenance.
- Units: common non-UCUM spellings (`lb`, `in`, `F`, `C`, `bpm`) are replaced by UCUM in `Quantity.code`; the original
  stays in `unit`. Each replacement is reported (`quantity.unit.normalized`). Unknown units pass through unchanged.

## Vital signs
- The organizer is a panel Observation (85353-1) with `hasMember`. Without an organizer time, the panel takes the span of
  its members' times.
- Blood pressure (8480-6 + 8462-4) is one Observation (85354-9) with two components and no value of its own. One reading
  alone is still a blood pressure Observation, with the other component marked `unknown` and the case reported: the
  validator treats the systolic and diastolic codes as blood pressure however they are used. Pulse oximetry (59408-5 or 2708-6) carries 3150-0 and 3151-8 as components.
- US Core profile claims are made per vital, and only when the unit is one the profile allows; otherwise the general
  `us-core-vital-signs` profile is claimed if the Observation is complete enough for it.
