# 0040. ImagingStudy from the DICOM Object Catalog

Status: accepted

## Context
USCDI v3 lists diagnostic imaging. C-CDA carries the study structure in the DICOM Object
Catalog section: a Study Act (6.2.6), Series Acts (22.4.63) and SOP Instance Observations (6.2.8).

## Decision
- Each Study Act becomes one `ImagingStudy` (status `available`, DICOM UID as identifier with
  system `urn:dicom:uid`, `started` from effectiveTime).
- Series and instances keep document order and are numbered from 1; counts are computed from
  what was mapped.
- Series modality comes from the DCM 121139 qualifier on the series code. If it is missing the
  modality carries the data-absent-reason extension and `imaging.modality.missing` is reported.
  A value is never guessed.
- A study without an id is not mapped (reported `imaging.study.no-uid`); a study without series
  is kept and reported `imaging.study.no-series`.
- No US Core profile is claimed: US Core 6.1.0 has none for ImagingStudy.
- `Code` in the model now carries `qualifiers`.

## Consequences
Narrative imaging reports remain text only (DocumentReference/Composition sections).
