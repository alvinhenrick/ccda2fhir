# 0024: Planned medications, instructions and procedure observations

Status: accepted. Found by running real-world samples (see `docs/coverage.md`). The IG's Procedures page covers the observation variant;
the others follow C-CDA and US Core.

- **Planned Medication Activity (4.42)** is mapped by the medication mapper exactly like a Medication Activity (4.16): a
  `MedicationRequest`, with the same intent rule (mood INT is an order, EVN a plan; ADR 0005).
- **Instruction (4.20)** at the top level of a section becomes a `ServiceRequest` with category Education (SNOMED 409073007), the
  statement code as `code`, and its text as `patientInstruction`. An Instruction nested in a medication or immunization stays in
  that resource's dosage or note, as before. A completed instruction is still a ServiceRequest (status `completed`); FHIR has no
  better home for "education that was given" short of a Procedure, which we do not guess.
- **Procedure Activity Observation (4.13)** with an event mood becomes an `Observation` in the `procedure` category, as the IG
  directs. With a planned or requested mood it is a `ServiceRequest` (ADR 0019).
- All three are now in the handled-template list, so none is reported as `entry.unmapped`. The 29 sample documents used to find
  them convert with no unmapped entries.
