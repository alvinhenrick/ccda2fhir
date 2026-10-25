# 0011: Patient demographics

Status: accepted. Source: IG page "CF-patient" and CSV.

- Every `name`, `addr` and `telecom` is kept. Name use: L is official, P is anonymous; other uses are left out. Address
  and telecom uses map H/HP, WP/DIR, MC (telecom), TMP/HV, BAD. The telecom system comes from the URI prefix
  (`tel:`, `fax:`, `mailto:`, `http`); anything else is `other`. Entries with no usable content are dropped.
- Race and ethnicity: `raceCode` and `sdtc:raceCode` (and the ethnicity pair) fill one extension each. Codes in the OMB
  list go to `ombCategory` (up to 5 for race, 1 for ethnicity), the rest to `detailed`. The required `text` is the first
  OMB display (we supply the official name when the document has none). Codes from other systems are dropped and
  reported; a nullFlavor of UNK or ASKU becomes that OMB category.
- Birth sex and gender identity are read from the Social History observations and become Patient extensions (M, F, UN to
  UNK for birth sex). The social mapper skips them.
- Guardians become `contact` entries. The provider organization is not mapped yet because the Patient is built before the
  shared organization registry exists.
