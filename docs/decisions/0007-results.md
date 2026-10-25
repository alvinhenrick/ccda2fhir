# 0007: Results to DiagnosticReport and Observation

Status: accepted. Source: IG page "CF-results", ConceptMaps `CF-ResultStatus` and `CF-ResultReportStatus` (identical).

- One Result Organizer is one `DiagnosticReport` (`result` lists the Observations) plus one `Observation` per Result
  Observation. Report time is the organizer's; if missing, the span of its observations.
- Category: the IG reads the LOINC class (needs a LOINC database). Until we have one, results are treated as laboratory
  and each is reported (`results.category.assumed-laboratory`). Imaging and other reports will be wrongly categorised
  until the LOINC class table exists. This is the known gap.
- US Core lab profiles are claimed only with category, code, a time and a value (or data absent reason).
- Not mapped yet, reported: specimen participants, the organizer's specimen collection procedure, comments.
- Value, range, status and performer rules are the shared ones in ADR 0006.
- `issued` (required by US Core for final reports) has no C-CDA source. It is taken from the latest author time, else
  the end of the effective time, when that reaches the second with an offset; reported as `results.issued.derived`.
  Without one, the US Core lab profile is not claimed.
