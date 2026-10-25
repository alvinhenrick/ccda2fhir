# 0046. Tribal affiliation and sex as Patient extensions

Status: accepted

## Decision
- A Tribal Affiliation Observation (4.506) becomes the US Core `us-core-tribal-affiliation`
  extension on the Patient, with the coded value in `tribalAffiliation`. `isEnrolled` is not set:
  the document does not say.
- A Sex Observation (4.507) becomes the US Core `us-core-sex` extension (`valueCode`). The US
  Core Sex value set is four SNOMED codes (female 248152002, male 248153007, unknown 184115007,
  nonbinary 33791000087105) plus data-absent-reason `asked-declined`; the codes were checked
  against the terminology server. Administrative codes M, F and UN are translated. Any other
  value is reported (`patient.sex.unrecognized`) and the observation stays a social history
  Observation, so nothing is lost.
- When an extension is produced, the matching Observation is not also emitted.
- The tribal code system OID 2.16.840.1.113883.5.140 maps to `v3-TribalEntityUS`.
- Sexual orientation and occupation stay Observations: US Core has Observation profiles for them.
