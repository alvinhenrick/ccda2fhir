# 0029: Assessment scales and non-medicinal supplies

Status: accepted. Found by running HL7's CDA-Examples through the converter.

- **Assessment Scale Observation (4.69)** (Glasgow Coma Scale, PHQ, a social needs screen) is a panel `Observation` with its score
  as the value and `hasMember` links to one `Observation` per Assessment Scale Supporting Observation (4.86, one question and
  its answer). The category comes from the section: functional status, mental status (cognitive-status, ADR 0023) or social
  history; anything else is `survey`, reported.
- A supporting observation can carry the score (`INT`) and the answer concept (`CO` or `CD`). The answer concept is the
  `valueCodeableConcept` and the score is the coding's `ordinalValue` extension, so neither is lost. `xsi:type="CO"` values
  are now read as coded values.
- **Non-Medicinal Supply Activity (4.50)** (a cane, eyeglasses): the Product Instance participant already became a `Device`
  (ADR 0018 family). The supply itself now becomes a `DeviceUseStatement` (status from `statusCode`, `timingPeriod` from
  the effective time; an unknown end is left out). A missing UDI on a supply is not a warning, since such devices do not
  need one.
