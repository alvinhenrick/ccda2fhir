# 0035. C-CDA R1.1, R2.1 and 3.0

Status: accepted

## Decision

- Templates are matched by root only; the `extension` (version) is never used. R1.1 documents (no versions), R2.1 and 3.0 therefore take the same path. A test removes every template version from four fixtures and checks the output is identical.
- Sections are found by their LOINC code or their entries, not by section template, so the R1.1/2.1 "entries optional" and "entries required" (`.1`) variants behave the same.
- C-CDA 3.0 social-history observations (sexual orientation 4.501, occupation 4.503, industry 4.504, tribal affiliation 4.506, sex 4.507, smoking status 4.511, pregnancy intention 4.281) map to social-history Observations.
- The two R1.1 sample documents convert with no unmapped entries.

## Not done

Templates 3.0 adds that are still reported as unmapped (never silently dropped): advance directives (4.48, 4.108), hospital admission/discharge diagnoses and medications (4.33 to 4.36), pre/post-operative diagnoses (4.51, 4.65), medication adherence and preference observations (4.508 to 4.510), average blood pressure (4.512), date of diagnosis (4.502), deceased observation (4.79).
