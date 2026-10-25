# 0047. Remaining C-CDA 3.0 observations

Status: accepted

## Decision
- Medication Adherence (4.508), Care Experience Preference (4.509) and Treatment Intervention
  Preference (4.510) become Observations in the `survey` category: they are what the patient
  reports or prefers.
- Advance Directive Existence (4.513), Interpreter Needed (4.515) and Age Range (4.516) become
  `social-history` Observations like the other 3.0 social observations (ADR 0035).
- The Average Blood Pressure Organizer (4.512) holds averaged readings, not a systolic and
  diastolic pair, so each member stays its own `vital-signs` Observation with its LOINC code.
  They are not folded into the blood pressure panel, which would misstate what was measured.
- A Deceased Observation (4.79) sets `Patient.deceasedDateTime` (or `deceasedBoolean` true when it
  has no usable date), only when the header's `sdtc:deceasedInd` and `deceasedTime` say nothing.
- No profile beyond US Core's own rules is claimed for these.
