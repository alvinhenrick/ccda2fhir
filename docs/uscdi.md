# USCDI coverage

Source: ONC USCDI v1 (July 2020 errata) and USCDI v3.1 (2025). "done" means a mapper and a test exist. "partial" means data is kept but not in the exact FHIR shape US Core wants. "todo" is a known gap. This table is a working checklist, not a conformance claim; the HL7 validator results in CI are the evidence for the done rows.

| Data class | Elements | Status | Notes |
|---|---|---|---|
| Patient demographics | names, previous name, DOB, date of death, race, ethnicity, preferred language, current/previous address, phone and type, email, birth sex | done | previous name and address via `period`; goldens `demographics`, `patient_details` |
| | related person name and relationship | done | guardians, participants, family contacts |
| | tribal affiliation, sex, occupation, industry | partial | mapped as social-history Observations (3.0 templates), not Patient extensions |
| | sexual orientation, gender identity | partial | v3 only; v3.1 removed them; Observation or reported, no Patient extension yet |
| Allergies and intolerances | substance (medication, drug class), reaction | done | |
| Problems | problems, date of diagnosis, date of resolution | done | onset and abatement |
| | SDOH problems/health concerns | todo | no SDOH category yet |
| Health status / assessments | health concerns, functional, disability, mental/cognitive, pregnancy, smoking status | done | |
| Assessment and plan of treatment | assessment and plan | done | CarePlan with activity links |
| | SDOH assessment, goals, interventions | todo | assessments and goals map, but without SDOH categories |
| Immunizations | immunizations | done | |
| Procedures | procedures | done | |
| | reason for referral | partial | `ServiceRequest.reasonCode`; the Referral act (4.140) is still reported as unmapped |
| Care team members | name, identifier, role, location, telecom | done | |
| Laboratory | tests, values/results, specimen type, result status | done | |
| Provenance | author organization, author time stamp | done | |
| Clinical notes | consultation, discharge summary, H&P, procedure, progress | done | Note Activity and unstructured documents |
| | imaging, laboratory and pathology report narratives | partial | narrative kept as text; no DiagnosticReport presentedForm |
| Medications | medications, dose, dose unit, indication, fill status | done | |
| Implantable device UDI | UDI | done | |
| Clinical tests | clinical test and result/report | done | Observation and DiagnosticReport |
| Diagnostic imaging | imaging test and report | todo | no ImagingStudy mapping |
| Vital signs | all vital signs and percentiles | done | |
| Encounter information | type, diagnosis, time, location, disposition | done | |
| Goals | patient goals | done | |
| Health insurance | coverage status and type, relationship, member, subscriber, group, payer | done | |
