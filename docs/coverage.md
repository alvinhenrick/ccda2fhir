# Coverage matrix

Status per C-CDA element: **done** (mapped and tested), **partial**, **reported** (not mapped; the
report says so), **todo**. "Validated" is filled per row once covered by a validator run.

**Validator status (CI workflow "FHIR validation", HL7 validator 7.0.3, FHIR 4.0.1, US Core 6.1.0):** every synthetic fixture
(39 of them, one or more per domain) converts to a bundle that the validator accepts with **0 errors** on every merged PR; the
workflow runs the validator once as a web service and takes about a minute. US Core profiles are declared in `meta.profile` only
when the required elements exist (ADR 0001, 0021). The remaining warnings are known: best-practice notes such as "observations
should have a performer", and value-set bindings that C-CDA data cannot always satisfy. The validator checks conformance, not
coverage: US Core "must support" completeness is tracked in `docs/uscdi.md`.

**Known gaps (everything else in this file is done or deliberately reported).** Each is reported by the converter, never dropped
silently.

- Supply request, authorization (4.123): the IG has no mapping for either (ADR 0056).
- CarePlan and health concern `addresses`; Goal progress as an R4 achievement code (only the document's own wording is kept).
- `reasonReference` to an Observation (only Conditions are linked).
- Coverage: plan name, authorizations, the member's period.
- Patient and related-person members of a care team; Location organization (see ADR 0054: C-CDA has no source for it).
- Note content from a narrative reference as XHTML rather than plain text; a source `DocumentReference`; `reference/externalDocument`.

- Advance directive witness participants (no template in C-CDA).
- Imaging, laboratory and pathology report narratives stay text.

## Problems (Problem Concern Act / Problem Observation) to Condition

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Concern `id`, `effectiveTime`, `priority` | not supported by target (IG) | n/a | | |
| Observation `id` | `identifier` | done | `standard_problem_follows_the_ig_example` | |
| Section LOINC code | `category` (CF-ProblemCategory) | done | `category_follows_the_section_code` | |
| Observation `code` | `category` | map not feasible (IG) | | |
| `effectiveTime/low` | `onsetDateTime` (+ DAR for nullFlavor) | done | `unknown_onset_becomes_a_data_absent_reason` | |
| `effectiveTime/high` | `abatementDateTime` (+ DAR for UNK) | done | `unknown_end_date_becomes_a_data_absent_reason` | |
| `value` | `code` | done | `missing_or_empty_values_are_reported_not_fatal` | |
| `negationInd` | `verificationStatus = refuted` | done | `negation_means_refuted` | |
| Problem Status value | `clinicalStatus` (CF-ProblemStatus) | done | `every_problem_status_in_the_ig_map` | |
| Concern `statusCode` | `clinicalStatus` (CF-ConditionClinicalStatus) | done | `concern_status_is_used_when_there_is_no_problem_status` | |
| Age Observation | `onsetAge` | done | `age_at_onset_is_used_only_without_an_onset_date` | |
| Date of Diagnosis | `assertedDate` extension | done | `date_of_diagnosis_becomes_asserted_date` | |
| Supporting observations (SPRT) | `evidence.detail` -> an Observation | done (ADR 0056) | `supporting_observation_becomes_evidence_detail`, golden `problems` | |
| Author | `recorder`, `recordedDate`, Practitioner, Organization, Provenance (ADR 0004) | done | `author.rs` tests; fixtures | |
| Comment Activity (4.64) | `note` on the parent resource | done | golden `small_templates`, `comment_becomes_a_note_on_the_parent` | |
| Prognosis, Priority Preference | not supported by target (IG) | n/a | | |

## Allergies (Allergy Concern Act / Allergy Intolerance Observation) to AllergyIntolerance

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Concern `id`, `effectiveTime`, `statusCode` | `statusCode` only as fallback for `clinicalStatus` (IG); others not supported by target | done | `status_precedence_and_defaults` | |
| Observation `id` | `identifier` | done | `drug_allergy_with_reaction_severity_and_criticality` | |
| `effectiveTime/low` | `onsetDateTime` | done | same | |
| `effectiveTime/high` | `allergyintolerance-abatement` extension | done | `end_date_becomes_the_abatement_extension` | |
| `value` | `type`, `category` (CF-AllergyIntoleranceType/Category) | done | `type_and_category_follow_the_ig_maps` | |
| `participant/.../playingEntity/code` | `code` | done | `drug_allergy_...` | |
| Negated, null substance | `code` via CF-NoKnownAllergies, else `code.text` | done | `negated_with_null_substance_uses_the_no_known_allergy_map` | |
| Negated, coded substance | `substanceExposureRisk` extension (no `code`; not US Core conformant, per IG) | done | `negated_with_a_coded_substance_uses_the_exposure_risk_extension` | |
| Allergy Status Observation | `clinicalStatus` (CF-AllergyStatus); absent = active | done | `status_precedence_and_defaults` | |
| Reaction Observation `value`, `effectiveTime` | `reaction.manifestation`, `reaction.onset` | done | `drug_allergy_...` | |
| Reaction `id` | `reaction.id` (IG: source value) | skipped: an element id cannot carry root+extension | | |
| Severity (reaction level, else allergy level) | `reaction.severity` (CF-Severity) | done | `reaction_severity_wins_over_...` | |
| Criticality Observation (82606-5) | `criticality` (CF-Criticality) | done | `drug_allergy_...` | |
| Author | `recorder`, `recordedDate`, Practitioner, Organization, Provenance (ADR 0004) | done | `author.rs` tests; fixtures | |
| Comment Activity (4.64) | `note` on the parent resource | done | golden `small_templates`, `comment_becomes_a_note_on_the_parent` | |

## Authors and supporting resources

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| `author/assignedAuthor/id`, `assignedPerson/name` | Practitioner (deduplicated) | done | `the_same_person_becomes_one_practitioner` | |
| `representedOrganization` | Organization, `Provenance.agent.onBehalfOf` | done | `organization_is_shared_and_linked_with_on_behalf_of` | |
| `author/time` | `recordedDate` (earliest), `Provenance.recorded` | done | `recorder_is_latest_and_recorded_date_is_earliest` | |
| `assignedAuthor/code` (role/specialty) | PractitionerRole | done | goldens `care_team`, `problems`, others | |
| `assignedAuthoringDevice` | Device, as a Provenance agent (never `recorder`) | done (ADR 0058) | `software_author_is_a_provenance_agent_but_never_the_recorder`, golden `goals` | |
| Author telecom, address | Practitioner.telecom, address | done | golden `header_participants` | |

## Immunizations (Immunization Activity, mood EVN) to Immunization

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| `negationInd` | `status = not-done` | done | `status_follows_negation_then_the_ig_map` | |
| `statusCode` | `status` (CF-ImmunizationStatus) | done | same | |
| `id` | `identifier` | done | `completed_immunization_follows_the_ig` | |
| `effectiveTime` (value or low) | `occurrenceDateTime` (DAR if absent) | done | `occurrence_uses_low_of_an_interval_or_a_data_absent_reason` | |
| `repeatNumber` | `protocolApplied.doseNumberPositiveInt` | done | `invalid_dose_number_is_reported` | |
| `routeCode`, `approachSiteCode` | `route`, `site` | done | `completed_immunization_follows_the_ig` | |
| `doseQuantity` (PQ) | `doseQuantity` | done | same | |
| `doseQuantity` (IVL) | not supported by target (IG) | reported, on purpose | `dose_range_and_an_empty_reaction_are_reported` | Immunization has one `doseQuantity`; a range would be a guess |
| `administrationUnitCode` | not supported by target (IG) | n/a | | |
| Material `code` | `vaccineCode` | done | `missing_vaccine_is_marked_unknown_with_a_warning` | |
| `lotNumberText` | `lotNumber` | done | `completed_immunization_follows_the_ig` | |
| `manufacturerOrganization` | `manufacturer` (Organization) | done | same | |
| `performer` | `performer` (function AP) | done | same | |
| `author` | `recorded`, Provenance | done | `authors_set_recorded_and_create_provenance` | |
| Not Given Reason (4.53) | `statusReason` | done | `reasons_split_by_template` | |
| Indication (4.19) | `reasonCode` | done | same | |
| Reaction (4.9) | `reaction.detail` (an Observation with `valueCodeableConcept`), `reaction.date` | done (ADR 0055) | `reaction_becomes_an_observation_referenced_from_the_immunization`, golden `immunizations` | |
| mood INT (planned) | MedicationRequest | done (ADR 0049) | `planned_immunization_becomes_a_medication_request` | |
| `primarySource` | not in C-CDA; omitted (R4/US Core 6.1.0 decide) | n/a | | |
| Comment Activity (4.64) | `note` on the parent resource | done | golden `small_templates`, `comment_becomes_a_note_on_the_parent` | |

## Medications (Medication Activity) to MedicationRequest (+ Medication)

See `docs/decisions/0005-medications.md` for the rules and the deliberate differences from the IG.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| `id` | `identifier` | done | golden `medications` | |
| `statusCode` | `status` (CF-MedicationStatus) | done | `status_map_follows_the_ig` | |
| `moodCode` INT / EVN | `intent` order / plan | done | `intent_follows_mood` | |
| `negationInd` | `doNotPerform` (not in IG) | done | `negation_sets_do_not_perform` | |
| `effectiveTime` IVL_TS | `timing.repeat.boundsPeriod` | done | golden | |
| `effectiveTime` PIVL_TS | `timing.repeat.period/periodMax/periodUnit` | done | `period_and_range` | |
| `effectiveTime` EIVL_TS | `timing.repeat.when/offset` | done | `offset_must_be_whole_minutes` | |
| `repeatNumber` | `timing.repeat.count` (INT only) | done | `repeat_number_is_a_count_for_orders_only` | |
| `routeCode`, `doseQuantity`, dose range, `rateQuantity` | `route`, `doseAndRate` | done | golden | |
| `maxDoseQuantity` | `maxDosePerPeriod` (IG: maxDosePerAdministration) | done, differs | golden | |
| Free Text Sig | `dosageInstruction.text` (IG: patientInstruction) | done, differs | golden | |
| Instruction | `dosageInstruction.patientInstruction` | done | golden | |
| `precondition` | `asNeededBoolean` (IG: not supported) | done, differs | `messages_never_contain_document_values` | |
| Consumable code | `medicationCodeableConcept` | done | golden | |
| Consumable with manufacturer | `Medication` + `medicationReference` | done | `manufacturer_makes_a_medication_resource` | |
| Indication | `reasonCode` | done | golden | |
| `performer` | `performer` (first only) | done | golden | |
| `author` | `requester`, `authoredOn`, Provenance | done | golden | |
| `approachSiteCode`, `administrationUnitCode`, drug vehicle | not supported (IG) | reported | golden report | |
| Supply order, Medication Dispense | `dispenseRequest` / MedicationDispense | done (ADR 0041) | golden `dispense` | |
| Planned Immunization Activity (4.120) | MedicationRequest | done (ADR 0049) | golden `planned-medication` | |

## Vital signs (Vital Signs Organizer / Vital Sign Observation) to Observation

See `docs/decisions/0006-vital-signs-and-observations.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Organizer | panel Observation 85353-1 + `hasMember` | done | `panel_links_every_member_and_takes_the_span_of_their_times` | |
| `id`, `statusCode`, `effectiveTime` | `identifier`, `status`, `effective[x]` | done | golden `vitals` | |
| `code`, `value` PQ | `code`, `valueQuantity` (UCUM normalized) | done | `non_ucum_units_are_replaced_in_code_and_kept_in_unit` | |
| Systolic + diastolic (or one alone) | blood pressure with components (a missing one is marked unknown) | done | `blood_pressure_is_one_observation_with_two_components` | |
| Pulse oximetry + O2 flow/concentration | pulse oximetry with components | done | `pulse_oximetry_carries_flow_and_concentration_as_components` | |
| `interpretationCode`, `methodCode`, `targetSiteCode` | `interpretation`, `method`, `bodySite` | done | golden | |
| `referenceRange` | `referenceRange` (normal only) | done | golden | |
| `author` | `performer` + Provenance | done | golden | |
| Per-vital US Core profile | `meta.profile` | done (unit-checked) | golden | |
| Author fallback from the organizer | `performer` | done | golden | |

## Results (Result Organizer / Result Observation) to DiagnosticReport + Observation

See `docs/decisions/0007-results.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Organizer `id`, `code`, `statusCode` | DiagnosticReport `identifier`, `code`, `status` | done | `organizer_is_a_report_that_lists_its_observations` | |
| Organizer `effectiveTime` | `effective[x]` (span of members if missing) | done | same | |
| Result Observations | `result` references + Observations | done | golden `results` | |
| Observation `value` by type | `value[x]` / `dataAbsentReason` | done | `value_types_map_to_value_elements` | |
| `interpretationCode`, `methodCode`, `targetSiteCode` | `interpretation`, `method`, `bodySite` | done | golden | |
| `referenceRange` | `referenceRange` (normal only) | done | `only_normal_reference_ranges_are_kept` | |
| Category from LOINC class | `category` | assumed laboratory and reported (`results.category.assumed-laboratory`) | golden | |
| `author` | `performer` + Provenance | done | golden | |
| Specimen, specimen collection procedure | Specimen | done (ADR 0018) | golden `location_and_specimen` | |

## Procedures (Procedure Activity Procedure / Act) to Procedure

See `docs/decisions/0008-procedures.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| `id`, `code` | `identifier`, `code` | done | golden `procedures` | |
| `statusCode`, `negationInd` | `status` (CF-ProcedureStatus; negation is not-done) | done | `status_map_and_negation` | |
| `effectiveTime` | `performedDateTime` / `performedPeriod` / data absent reason | done | `point_period_and_missing_times` | |
| `targetSiteCode` | `bodySite` | done | golden | |
| `performer` | `performer.actor` (Practitioner / Organization) | done | golden | |
| `author` | `recorder` + Provenance | done | golden | |
| Indication | `reasonCode` | done | golden | |
| Act variant | Procedure | done | `the_act_variant_maps_the_same_way` | |
| `methodCode`, `priorityCode`, approach | not supported (IG) | reported | | |
| Service Delivery Location | `location` | done (ADR 0018) | golden `location_and_specimen` | |
| Encounter link, comment | `encounter`, `note` | done (ADR 0025, 0034) | golden | |
| Instruction (4.20) in a procedure | `followUp` (code and text) | done (ADR 0057; not in the IG) | `nested_instruction_becomes_follow_up`, golden `procedures` | |
| Procedure Activity Observation | Observation | done (ADR 0008) | golden | |
| Planned procedure (mood INT/RQO/PRP) | ServiceRequest | done | `planned_procedure_becomes_a_plan` | |

## Encounters (Encounter Activity) to Encounter

See `docs/decisions/0009-encounters.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| `id` | `identifier` | done | golden `encounters` | |
| `code` (v3 ActCode, or CPT E/M range) | `class` | done | `cpt_gives_class_and_stays_in_type`, `act_code_gives_class_and_leaves_type` | |
| `code` (the rest) | `type` | done | same | |
| `statusCode` / `effectiveTime` | `status` (time first, CF-EncounterStatus otherwise) | done | `time_decides_status_unless_the_status_code_says_otherwise` | |
| `effectiveTime` | `period` | done | `point_time_is_a_period_with_both_ends` | |
| `sdtc:dischargeDispositionCode` | `hospitalization.dischargeDisposition` | done | `discharge_disposition_is_read_from_the_sdtc_element` | |
| `performer` | `participant.individual` | done (function code todo) | golden | |
| Indication | `reasonCode`, and `reasonReference` to the Condition with the same id | done (ADR 0050) | golden `reasons` | |
| Encounter Diagnosis | Condition (encounter-diagnosis) + `diagnosis.condition` | done | `encounter_diagnoses_become_conditions_linked_both_ways` | |
| Location participant | `location` | done (ADR 0018) | golden | |
| Header `encompassingEncounter`, same encounter seen twice | one Encounter | done (ADR 0026) | golden | |
| Links from procedures, conditions and results to the encounter | `encounter` | done (ADR 0025) | golden | |
| Planned encounter (mood INT) | ServiceRequest | done | golden `planned_activities` | |

## Social history (Social History / Smoking / Tobacco / Pregnancy Observation) to Observation

See `docs/decisions/0010-social-history.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Social History Observation | Observation, category `social-history` | done | golden `social` | |
| Smoking Status MU / Tobacco Use | Observation; US Core smoking status when coded and dated | done | `smoking_status_is_a_social_history_observation_with_a_date` | |
| Start-only `effectiveTime` | `effectiveDateTime` | done | same | |
| Pregnancy Observation | Pregnancy status / intent Observation (LOINC code) | done | `pregnancy_uses_the_loinc_code_one_time_and_an_edd_component` | |
| Estimated delivery date | component 11778-8 | done | same | |
| Unknown pregnancy value | v3 NullFlavor UNK coding | done | `unknown_pregnancy_value_is_a_null_flavor_coding` | |
| Birth sex, gender identity, tribal affiliation, sex | Patient extensions | done (ADR 0011, 0046) | `birth_sex_and_gender_identity_come_from_social_history`, `tribal_affiliation_and_sex_become_patient_extensions` | |
| SDOH categories, assessment scales | `sdoh` category by code list; scales as panels | partial | golden `sdoh` | |

## Patient demographics (recordTarget) to Patient

See `docs/decisions/0011-patient-demographics.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| `id` | `identifier` | done | `maps_identifiers_name_gender_and_birth_date` | |
| `name` (all, with use, prefix, suffix) | `name` | done | `names_addresses_and_telecoms_keep_their_parts_and_uses` | |
| `addr`, `telecom` | `address`, `telecom` (system from the URI prefix) | done | same | |
| `administrativeGenderCode` | `gender` | done | `gender_follows_the_ig_concept_map` | |
| `birthTime` | `birthDate` | done | `invalid_birth_time_is_omitted_with_a_warning` | |
| `maritalStatusCode` | `maritalStatus` | done | `language_marital_status_and_guardian` | |
| `raceCode`, `sdtc:raceCode` | US Core race extension | done | `race_and_ethnicity_become_us_core_extensions` | |
| `ethnicGroupCode`, `sdtc:ethnicGroupCode` | US Core ethnicity extension | done | same | |
| Birth Sex Observation | US Core birth sex extension | done | `birth_sex_and_gender_identity_come_from_social_history` | |
| Gender Identity Observation | US Core gender identity extension | done | same | |
| `languageCommunication` | `communication` | done | `language_marital_status_and_guardian` | |
| `guardian` | `contact` | done | same | |
| `providerOrganization` | `managingOrganization` | done (ADR 0031) | golden `patient_details` | |
| Sexual orientation | `Observation` (US Core Sexual Orientation) | done (ADR 0035) | golden `social_3_0` | |
| Religious affiliation | `patient-religion` extension | done (ADR 0031) | golden `patient_details` | |
| Birthplace | not supported (IG) | not mapped, not yet reported | | |

## Clinical notes (Note Activity) to DocumentReference

See `docs/decisions/0012-notes.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| `id` | `identifier` | done | golden `notes` | |
| `code` translation (LOINC) | `type` (root 34109-9 if none) | done | `narrative_reference_becomes_plain_text_content` | |
| fixed | `category` clinical-note, `status` current | done | same | |
| `text` (embedded base64 + mediaType) | `content.attachment` | done | `embedded_base64_keeps_its_media_type` | |
| `text/reference` (narrative) | `content.attachment` as text/plain | done (XHTML is todo) | `narrative_reference_becomes_plain_text_content` | |
| `effectiveTime` | `context.period` | done | `author_and_time_set_author_date_and_period` | |
| `author` | `author`, `date`, Provenance | done | same | |
| Encounter link | `context.encounter` | done (ADR 0025) | golden | |
| `reference/externalDocument` | `relatesTo` | todo | | |
| The document itself | Composition | done (ADR 0020); a source DocumentReference is todo | golden `composition` | |

## Devices (Product Instance / UDI) to Device

See `docs/decisions/0013-devices.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| `participantRole/id` (FDA UDI root) | `udiCarrier.carrierHRF` (+ `deviceIdentifier` for GS1) | done | `udi_gives_carrier_with_device_identifier_and_type` | |
| `playingDevice/code` | `type` | done | same | |
| Same UDI seen twice | one Device | done | `the_same_udi_twice_is_one_device` | |
| Lot, serial, dates in the UDI | `lotNumber`, `serialNumber`, `expirationDate`, `manufactureDate` | done (GS1 AIs 10, 21, 17, 11; ADR 0042) | golden `devices` | |
| Link from the procedure | `Procedure.focalDevice` | done (ADR 0042) | golden `devices` | |
| Non-implantable Device uses (assessment scales, software) | other Devices | todo (supplies done: DeviceUseStatement, ADR 0029) | | |

## Goals to Goal

See `docs/decisions/0014-goals.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Goal Observation code / value | `description` | done | `description_is_the_code_or_the_value_for_assertion` | |
| `statusCode` | `lifecycleStatus` | done | `status_map` | |
| `effectiveTime` | `startDate`, `target.dueDate` | done | `dates_become_start_and_target` | |
| `author` | `expressedBy`, Provenance | done | | |
| Progress Toward Goal (4.110) | `achievementStatus` (the document's code or text, not guessed into an R4 code) | done (ADR 0048) | `progress_and_priority_are_mapped`, golden `goals` | |
| Goal target | `target` | done (ADR 0014) | golden `goals` | |
| Priority Preference (4.143) | `priority` | done (ADR 0048) | `progress_and_priority_are_mapped`, golden `goals` | |
| Nested Problem Observation | `addresses` | done (ADR 0048) | golden `goals` | `goal.addresses.unresolved` when the problem is not in the document |
| Links to Care Plan, Health Concerns | `addresses` | todo | | |

## Plan of Treatment to CarePlan

See `docs/decisions/0015-care-plan.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Plan of Treatment section | `CarePlan` (intent plan, category assess-plan) | done | `plan_has_required_fields_and_escaped_narrative` | |
| Section narrative | `text.div` | done | same | |
| Goals in the document | `goal` | done | `plan_points_at_goals` | |
| Planned activities (4.39-4.41, 4.44) | ServiceRequest | done | see below | |
| Planned activities as `CarePlan.activity` | `activity.reference` | done | golden `planned_activities` | |
| Health Concerns | `addresses` | todo | | |
| Care team | `CareTeam` | done (ADR 0016) | golden `care_team` | |

## Care Team to CareTeam

See `docs/decisions/0016-care-team.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Care Team Organizer | `CareTeam` | done | `member_becomes_participant_with_role_and_period` | |
| `statusCode` | `status` | done | `status_map_and_fallback` | |
| Member performer | `participant.member` (Practitioner) | done | same as first | |
| Performer code | `participant.role` | done | same | |
| Member effectiveTime | `participant.period` | done | same | |
| Member role (the performer's role code) | `role` | done (ADR 0016) | `care_team` tests | |
| `functionCode` (performer or act) | `role` (one; the specialty is the fallback) | done (ADR 0053) | `function_code_is_the_role_and_the_specialty_the_fallback`, golden `care-team` | |
| Patient and related-person members | RelatedPerson | todo | | |
| PractitionerRole, Location | PractitionerRole, Location | done | golden `care_team` | |

## Insurance (Policy Activity) to Coverage

See `docs/decisions/0017-coverage.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Policy Activity | `Coverage` | done | `policy_becomes_coverage` | |
| Policy code (SOPT) | `type` | done | same | |
| Covered party id / role | `identifier`, `relationship` | done | same | |
| Policy holder id | `subscriberId` | done | same | |
| Policy ids | `class` (group) | done | same | |
| Payer performer | `payor` (Organization) | done | `missing_payer_and_role_are_reported` | |
| Subscriber id, group | `subscriberId`, `class` | done (ADR 0017) | golden `coverage` | |
| Subscriber as a person | `subscriber` (Patient when the covered party is SELF, otherwise a RelatedPerson) | done (ADR 0051) | `self_subscriber_is_the_patient`, golden `coverage` | |
| Plan name | `class.name` | todo | | |
| Authorization (4.123) | none in the IG | reported, on purpose (ADR 0056) | `authorization_is_reported_not_mapped` | no R4 resource fits without guessing |

## Locations and Specimens

See `docs/decisions/0018-location-and-specimen.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Service Delivery Location (4.32) | `Location` | done | `named_location_is_created_once` | |
| Location on an encounter | `Encounter.location` | done | golden `location_and_specimen` | |
| Location on a procedure | `Procedure.location` | done | | |
| `specimenRole` | `Specimen` | done | `name_only_specimen_gets_text_type_and_subject` | |
| Specimen `desc`, `quantity` | `note`, `collection.quantity` | done | `description_becomes_a_note` | |
| Specimen Collection Procedure | `collection.bodySite`, `collectedDateTime` | done | golden | |
| Specimen on organizer / observation | `DiagnosticReport.specimen`, `Observation.specimen` | done | golden | |
| Specimen Condition Observation (4.421) | `condition` | done (ADR 0054) | golden `location-specimen` | |
| Location status, organization | `status`, `managingOrganization` | no source in C-CDA (ADR 0054) | | the Service Delivery Location role has no status or organization |

## Planned activities to ServiceRequest

See `docs/decisions/0019-service-requests.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Planned Procedure (4.41), Procedure Activity with non-EVN mood | `ServiceRequest` | done | `planned_procedure_becomes_a_plan` | |
| Planned Encounter (4.40) | `ServiceRequest` | done | golden `planned_activities` | |
| Planned Observation (4.44) | `ServiceRequest` | done | `missing_status_is_assumed_and_reported` | |
| Planned Act (4.39) | `ServiceRequest` | done | `planned_act_has_no_assumed_category` | |
| Mood | `intent` | done | `mood_decides_intent` | |
| statusCode | `status` | done | | |
| Indication, targetSiteCode | `reasonCode`, `bodySite` | done | golden | |
| Author | `requester`, `authoredOn`, Provenance | done | golden | |
| Planned medication | MedicationRequest | done (ADR 0024) | golden `planned_medication` | |
| Planned supply | SupplyRequest | todo | | |
| Instructions (4.20) | `patientInstruction` | done (ADR 0024); Communication is todo | golden `planned_medication` | |

## Document header to Composition

See `docs/decisions/0020-composition.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| `ClinicalDocument/id` | `identifier` | done | golden `composition` | |
| `code` | `type` | done | `header_fields_and_sections` | |
| `title` | `title` | done | same | |
| `effectiveTime` | `date` | done | same | |
| `author` | `author` (Practitioner) | done | `missing_author_means_no_composition` | |
| `custodian` | `custodian` | done | `header_fields_and_sections` | |
| `confidentialityCode` | `confidentiality` | done | `unknown_confidentiality_is_reported` | |
| `languageCode` | `language` | done | `header_fields_and_sections` | |
| Sections | `section` (title, code, text, entry) | done | same | |
| Entries in a section | `section.entry` | done (ADR 0020) | golden `composition` | |
| `legalAuthenticator`, `authenticator` | `attester` | done (ADR 0030) | golden `composition` | |
| `componentOf/encompassingEncounter` | `encounter` | done (ADR 0026) | golden `composition` | |
| `documentationOf/serviceEvent` | `event` | done (ADR 0030) | golden `composition` | |
| `relatedDocument` | `relatesTo` | done (ADR 0030) | golden `composition` | |

## Health Concerns

See `docs/decisions/0022-health-concerns.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Problem Concern Act in the Health Concerns section | `Condition` (category health-concern) | done | `category_follows_the_section_code` | |
| Health Concern Act (4.132) pointing by Entry Reference (4.122) | adds category `health-concern` | done | `a_pointed_at_condition_gains_the_health_concern_category_once` | |
| Problem Observation inside a Health Concern Act | `Condition` (health-concern) | done | `a_wrapped_problem_observation_becomes_a_health_concern_condition` | |
| Unresolved pointer, empty concern | report entries | done | `an_unresolved_pointer_is_reported` | |
| Pointer at allergy, result, social history | category / `addresses` | todo (the pointer is reported) | | |
| Risk Concern Act (4.136) | `Condition` (like a Problem Concern Act) | done | golden `small_templates` | |

## Functional, cognitive, mental and health status

See `docs/decisions/0023-status-assessments.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Functional Status Organizer (4.66) | panel `Observation` + `hasMember` | done | `organizer_is_a_panel_with_members_and_the_functional_status_category` | |
| Functional Status / Problem Observation, Self-Care Activities | `Observation` (functional-status) | done | same | |
| Cognitive Status Organizer / Observation (4.75 / 4.74) | `Observation` (cognitive-status) | done | `cognitive_and_mental_and_health_status_categories` | |
| Mental Status Organizer / Observation / Problem | `Observation` (cognitive-status, assumed) | done | same | |
| Health Status Observation (4.5) | `Observation` (survey) | done | same | |
| Assessment Scale Observation (4.69) | `Observation` + components | done (ADR 0029) | golden `assessments` | |
| Caregiver Characteristics (4.72), Disability Status (4.505) | `Observation` (functional-status) | done | golden `small_templates` | |

## Planned medications, instructions, procedure observations

See `docs/decisions/0024-planned-medication-instructions.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Planned Medication Activity (4.42) | `MedicationRequest` | done | golden `planned_medication` | |
| Instruction (4.20), top level | `ServiceRequest` (education, `patientInstruction`) | done | `an_instruction_is_an_education_request_with_the_text_as_patient_instruction` | |
| Procedure Activity Observation (4.13), event | `Observation` (procedure) | done | `procedure_activity_observation_is_an_observation_in_the_procedure_category` | |
| Procedure Activity Observation (4.13), planned | `ServiceRequest` | done | `a_planned_procedure_activity_observation_is_a_request` | |

## Encounter links

See `docs/decisions/0025-encounter-links.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Nested Encounter Activity (id only) in an entry | `encounter` on Condition, Observation, Procedure, DiagnosticReport, MedicationRequest, Immunization, ServiceRequest, CarePlan | done | golden `encounter_links` | |
| Encounter id not in the document | info `encounter.link.unresolved` | done | golden `encounter_links` | |
| `componentOf/encompassingEncounter` | `Encounter` and `Composition.encounter` (ADR 0026) | done | goldens `encompassing_encounter`, `encompassing_encounter_already_in_the_body` | |

## Family history

See `docs/decisions/0028-family-history.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Family History Organizer (4.45) | `FamilyMemberHistory` | done | golden `family_history` | |
| Family History Observation (4.46) | `condition` (`code`, `onsetAge`, `contributedToDeath`) | done | golden `family_history` | |
| Age Observation (4.31), Death Observation (4.47) | `onsetAge`, `contributedToDeath` | done | golden `family_history` | |
| Negated family history | `note` + report | done | golden `family_history` | |

## Assessment scales and supplies

See `docs/decisions/0029-scales-and-supplies.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Assessment Scale Observation (4.69) + Supporting Observation (4.86) | panel `Observation` + member `Observation`s (`ordinalValue`) | done | golden `scales_and_supplies` | |
| Non-Medicinal Supply Activity (4.50) | `Device` + `DeviceUseStatement` | done | golden `scales_and_supplies` | |

## Header participants

See `docs/decisions/0030-header-participants.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| `legalAuthenticator`, `authenticator` | `Composition.attester` | done | golden `header_participants` | |
| `participant/associatedEntity` | `Patient.contact` | done | golden `header_participants` | |
| `dataEnterer`, `informant` | `Provenance` (`RelatedPerson` for relatives) | done | golden `header_participants` | |
| `relatedDocument` | `Composition.relatesTo` | done | golden `header_participants` | |
| `telecom`/`addr` of authors and organizations | `Practitioner`/`Organization` | done | golden `header_participants` | |
| `informationRecipient`, `inFulfillmentOf`, `authorization` | | open | | |

## Data-loss audit

`cargo run --release -p ccda2fhir --example audit -- <files>` checks every attribute and text value of the source documents (outside
section narrative) for presence in the converted bundle, and prints counts by element path (no values). Misses are candidates, not
proof: a status mapped to another vocabulary is expected. The first run over 29 sample documents found these, all since fixed:

- 19 of 29 documents had no header author. The Composition is always built, with the author marked absent (ADR 0020).
- `authenticator`, `legalAuthenticator`, `dataEnterer`, `informant`, `participant/associatedEntity` and the `telecom` and
  `addr` of authors and organizations (ADR 0030).
- Section titles and codes go with the Composition; `originalText` references are resolved from the narrative; software and
  organization authors become a Device or Organization.

Still not mapped, and reported where they are entries: `informationRecipient`, `inFulfillmentOf`, `authorization` (no
Composition element), and translations on observation codes.

## Running real-world samples

`cargo run --release -p ccda2fhir --example corpus -- <files>` converts a set of documents and counts what was not mapped (report
code `entry.unmapped`, with the template OID). On the 29 sample documents in the Medplum repository (ONC 170.315(b)(1) samples and
small unit samples) nothing fails to convert and no entry is unmapped. Gaps it found earlier are now covered: Health Concerns,
status assessments, planned medications, instructions, procedure observations, hospital-course wrappers, advance directives and
the C-CDA 3.0 observations.

## Policy questions

- **Display names.** Settled in ADR 0027: the official display replaces a wrong one when the code is in our checked table, and the
  document's wording is kept as `text`; unknown codes keep the document's display. The table grows from validator findings.
- **Prior art on codes** (read from source): Medplum and Microsoft both pass `displayName` through unchanged. Microsoft also maps
  a value-set OID mistakenly used as a code system to SNOMED, a robustness hack worth considering as an opt-in repair.
- **OID table.** Entries are added as fixtures need them; the table still needs a full review against the HL7 OID registry.

## Patient completeness

See `docs/decisions/0031-patient-completeness.md`.

| C-CDA | FHIR | Status | Test |
|---|---|---|---|
| `sdtc:deceasedInd`, `sdtc:deceasedTime` | `deceased[x]` | done | golden `patient_details` |
| `name/validTime`, `addr/useablePeriod` | `period` | done | golden `patient_details` |
| `religiousAffiliationCode` | `patient-religion` extension | done | golden `patient_details` |
| `modeCode`, `proficiencyLevelCode` | `patient-proficiency` extension | done | golden `patient_details` |
| `id@assigningAuthorityName` | `Identifier.assigner.display` | done | golden `patient_details` |
| `providerOrganization` | `managingOrganization` | done | golden `patient_details` |

## Unstructured documents (nonXMLBody)

See `docs/decisions/0032-unstructured-documents.md`.

| C-CDA | FHIR | Status | Test |
|---|---|---|---|
| `nonXMLBody/text` (B64 or TXT) | `DocumentReference.content.attachment.data` | done | golden `unstructured` |
| `nonXMLBody/text/reference` | `attachment.url` (reported) | done | `reference_only_becomes_a_url_and_is_reported` |

## Generated narrative

See `docs/decisions/0033-generated-narrative.md`. Every resource without `text` gets a generated `text.div` (status `generated`), which clears the best-practice `dom-6` warning. Resources that already have narrative (Composition sections, CarePlan) keep it. Golden: all.


Appointment: a Planned Encounter with mood APT becomes an `Appointment` (golden `small_templates`, `instant_becomes_start`).

## C-CDA versions (R1.1, R2.1, 3.0)

See `docs/decisions/0035-template-versions.md`. Template matching ignores versions (test `template_versions_do_not_change_the_output`).

| Template | FHIR | Status | Test |
|---|---|---|---|
| Sexual Orientation (4.501), Occupation (4.503), Industry (4.504), Tribal Affiliation (4.506), Sex (4.507), Smoking Status 3.0 (4.511), Pregnancy Intention (4.281) | `Observation` (social-history) | done | golden `social_3_0`, `every_3_0_template_becomes_a_social_history_observation` |

## Referral

| C-CDA | FHIR | Status | Test |
|---|---|---|---|
| Referral (4.140) | `ServiceRequest` (category Patient referral) | done | golden `referral` |
| `participant typeCode="REFT"` | `performer` (Practitioner), `performerType` | done | golden `referral` |
| `priorityCode` (R, UR, A, S, EM) | `priority` | done | golden `referral` |

## Interventions and SDOH goals

See `docs/decisions/0039-interventions-sdoh.md`.

| C-CDA | FHIR | Status | Test |
|---|---|---|---|
| Intervention Act (4.131) and its Procedure Activity | `Procedure` | done | golden `sdoh_interventions` |
| Planned Intervention Act (4.146) and its planned activity | `ServiceRequest` | done | golden `sdoh_interventions` |
| SDOH Z-code on an intervention or goal | `sdoh` category (Procedure: social service procedure) | done | golden `sdoh_interventions` |

## Imaging (DICOM Object Catalog)

| C-CDA | FHIR | Status | Test |
|---|---|---|---|
| Study Act (6.2.6) | `ImagingStudy` | done (ADR 0040) | golden `imaging` |
| Series Act (22.4.63) | `ImagingStudy.series` (modality from DCM 121139) | done; missing modality is marked absent and reported | golden `imaging` |
| SOP Instance Observation (6.2.8) | `series.instance` | done | golden `imaging` |

## Supply orders and dispenses

| C-CDA | FHIR | Status | Test |
|---|---|---|---|
| Supply Order (4.17) | `MedicationRequest.dispenseRequest` | done (ADR 0041); a second order is reported | golden `dispense` |
| Medication Dispense (4.18) | `MedicationDispense` | done (ADR 0041) | golden `dispense` |

## Hospital course, directives and the remaining 3.0 templates

| C-CDA | FHIR | Status | Test |
|---|---|---|---|
| Admission / Discharge Diagnosis (4.34, 4.33), Pre / Postprocedure Diagnosis (4.65, 4.51) | `Condition` (encounter-diagnosis plus the role as a second category) | done (ADR 0043) | golden `hospital_course` |
| Admission / Discharge Medication (4.36, 4.35) | `MedicationRequest` (discharge category) | done (ADR 0043) | golden `hospital_course` |
| Advance Directive Observation / Organizer (4.48, 4.108) | `Consent` (scope adr) | done (ADR 0044) | golden `advance_directive` |
| Medication Adherence, Care Experience / Treatment Intervention Preference (4.508 to 4.510) | `Observation` (survey) | done (ADR 0047) | `adherence_and_preferences_are_survey_observations` |
| Advance Directive Existence, Interpreter Needed, Age Range (4.513, 4.515, 4.516) | `Observation` (social-history) | done (ADR 0047) | `every_3_0_template_becomes_a_social_history_observation` |
| Average Blood Pressure Organizer (4.512) | `Observation` (vital-signs), one per average | done (ADR 0047) | `average_blood_pressure_keeps_each_average_as_its_own_vital_sign` |
| Deceased Observation (4.79) | `Patient.deceased[x]` | done (ADR 0047) | `deceased_observation_sets_deceased_when_the_header_is_silent` |
| Date of Diagnosis (4.502) | `assertedDate` extension | done | golden `problems` |

## Provenance of the conversion

| Source | FHIR | Status | Test |
|---|---|---|---|
| The whole document | one derivation `Provenance` (targets, source Composition, assembler Device) | done (ADR 0045) | every golden |

