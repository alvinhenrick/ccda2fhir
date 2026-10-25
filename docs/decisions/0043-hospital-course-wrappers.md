# 0043. Hospital course wrappers

Status: accepted

## Context
Admission (4.34), Discharge (4.33), Preprocedure (4.65) and Postprocedure (4.51) Diagnosis, and
Admission (4.36) and Discharge (4.35) Medication are acts that wrap ordinary Problem
Observations and Medication Activities. They were only reported as unmapped.

## Decision
- The wrapped Problem Observations become Conditions with the `encounter-diagnosis` category.
  The wrapper's own code (for example LOINC 46241-6, admission diagnosis) is added as a second
  category, so the role is kept without inventing a vocabulary.
- The wrapped Medication Activities map as usual. A Discharge Medication gets the
  `medicationrequest-category` code `discharge`. Admission Medication adds nothing: R4 has no
  admission category.
- An empty diagnosis wrapper is reported (`condition.concern.no-observation`).
