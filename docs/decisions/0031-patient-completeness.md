# 0031. Patient completeness

Status: accepted

## Context

An audit of the HL7 header examples showed the Patient dropped several header elements.

## Decision

- `deceasedInd` / `deceasedTime` (sdtc) map to `deceasedBoolean` / `deceasedDateTime` (time wins).
- `name/validTime` maps to `HumanName.period`; `addr/useablePeriod` to `Address.period`.
- `religiousAffiliationCode` maps to the `patient-religion` extension.
- Language `modeCode` and `proficiencyLevelCode` map to the `patient-proficiency` extension.
- `id@assigningAuthorityName` maps to `Identifier.assigner.display` (every identifier, not only the patient's).
- `providerOrganization` becomes an Organization referenced by `Patient.managingOrganization`.

Nothing is invented: absent elements are simply not emitted.

## Consequences

Golden `patient_details` covers all of the above and passes the HL7 validator.
