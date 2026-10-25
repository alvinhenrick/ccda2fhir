# Coverage matrix

Status per C-CDA element: **done** (mapped and tested), **partial**, **reported** (not mapped; the
report says so), **todo**. "Validated" is filled per row once covered by a validator run.

**Validator status (CI workflow "FHIR validation", HL7 validator 7.0.3, FHIR 4.0.1, US Core 6.1.0):** the
`problems` and `allergies` fixtures produce **0 errors** with the US Core profiles declared in `meta.profile`.
Remaining warnings are the best-practice `dom-6` (resources lack narrative). Timestamps with a time and
offset pass, including one whose missing seconds we fill with `:00`.
Not yet proven: US Core "must support" completeness (the validator checks conformance, not coverage),
and only the two fixtures above have been validated.

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
| Supporting observations (SPRT) | `evidence.detail` | reported | `supporting_observations_are_reported_as_unsupported` | |
| Author | `recorder`, `recordedDate`, Practitioner, Organization, Provenance (ADR 0004) | done | `author.rs` tests; fixtures | |
| Comment Activity | `note` | todo | | |
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
| Comment Activity | `note` | todo | | |

## Authors and supporting resources

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| `author/assignedAuthor/id`, `assignedPerson/name` | Practitioner (deduplicated) | done | `the_same_person_becomes_one_practitioner` | |
| `representedOrganization` | Organization, `Provenance.agent.onBehalfOf` | done | `organization_is_shared_and_linked_with_on_behalf_of` | |
| `author/time` | `recordedDate` (earliest), `Provenance.recorded` | done | `recorder_is_latest_and_recorded_date_is_earliest` | |
| `assignedAuthor/code` (role/specialty) | PractitionerRole | todo | | |
| `assignedAuthoringDevice` | Device | reported | `software_authors_are_noted_not_mapped` | |
| Author telecom, address | Practitioner.telecom, address | todo | | |

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
| `doseQuantity` (IVL) | not supported by target (IG) | reported | `dose_range_and_reaction_are_reported` | |
| `administrationUnitCode` | not supported by target (IG) | n/a | | |
| Material `code` | `vaccineCode` | done | `missing_vaccine_is_marked_unknown_with_a_warning` | |
| `lotNumberText` | `lotNumber` | done | `completed_immunization_follows_the_ig` | |
| `manufacturerOrganization` | `manufacturer` (Organization) | done | same | |
| `performer` | `performer` (function AP) | done | same | |
| `author` | `recorded`, Provenance | done | `authors_set_recorded_and_create_provenance` | |
| Not Given Reason (4.53) | `statusReason` | done | `reasons_split_by_template` | |
| Indication (4.19) | `reasonCode` | done | same | |
| Reaction (4.9) | `reaction` (Observation) | reported | `dose_range_and_reaction_are_reported` | |
| mood INT (planned) | MedicationRequest | reported | `planned_immunizations_are_reported_not_mapped` | |
| `primarySource` | not in C-CDA; omitted (R4/US Core 6.1.0 decide) | n/a | | |
| Comment Activity | `note` | todo | | |

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
| Supply order, Medication Dispense | `dispenseRequest` / MedicationDispense | reported, todo | | |
| Planned immunization (mood INT) | MedicationRequest | todo | | |

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
| Category from LOINC class | `category` | assumed laboratory, todo | | |
| `author` | `performer` + Provenance | done | golden | |
| Specimen, specimen collection procedure | Specimen | reported, todo | | |

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
| Service Delivery Location | `location` | reported, todo (needs Location) | | |
| Encounter link, comment, instruction (`followUp`) | `encounter`, `note`, `followUp` | todo | | |
| Procedure Activity Observation | Observation | reported, todo | | |
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
| Indication | `reasonCode` (`reasonReference` to a problem todo) | partial | golden | |
| Encounter Diagnosis | Condition (encounter-diagnosis) + `diagnosis.condition` | done | `encounter_diagnoses_become_conditions_linked_both_ways` | |
| Location participant | `location` | reported, todo (needs Location) | | |
| Header `encompassingEncounter`, same encounter seen twice | one Encounter | todo | | |
| Links from procedures, conditions and results to the encounter | `encounter` | todo | | |
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
| Birth sex, gender identity, tribal affiliation | Patient extensions | reported, todo (with Patient demographics) | `birth_sex_is_left_for_the_patient_and_reported` | |
| SDOH categories, assessment scales | other categories / Observation panels | todo | | |

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
| `providerOrganization` | `managingOrganization` | todo | | |
| Sex, sexual orientation, tribal affiliation | US Core extensions / Observations | todo | | |
| Religious affiliation, birthplace | not supported (IG) | not mapped, not yet reported | | |

## Clinical notes (Note Activity) to DocumentReference

See `docs/decisions/0012-notes.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| `id` | `identifier` | done | golden `notes` | |
| `code` translation (LOINC) | `type` (root 34109-9 if none) | done | `narrative_reference_becomes_plain_text_content` | |
| fixed | `category` clinical-note, `status` current | done | same | |
| `text` (embedded base64 + mediaType) | `content.attachment` | done | `embedded_base64_keeps_its_media_type` | |
| `text/reference` (narrative) | `content.attachment` as text/plain | done (XHTML todo) | `narrative_reference_becomes_plain_text_content` | |
| `effectiveTime` | `context.period` | done | `author_and_time_set_author_date_and_period` | |
| `author` | `author`, `date`, Provenance | done | same | |
| Encounter link | `context.encounter` | todo | | |
| `reference/externalDocument` | `relatesTo` | todo | | |
| The document itself | Composition / DocumentReference | todo | | |

## Devices (Product Instance / UDI) to Device

See `docs/decisions/0013-devices.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| `participantRole/id` (FDA UDI root) | `udiCarrier.carrierHRF` (+ `deviceIdentifier` for GS1) | done | `udi_gives_carrier_with_device_identifier_and_type` | |
| `playingDevice/code` | `type` | done | same | |
| Same UDI seen twice | one Device | done | `the_same_udi_twice_is_one_device` | |
| Lot, serial, dates in the UDI | `lotNumber`, `serialNumber`, `expirationDate` | todo | | |
| Link from the procedure | `Procedure.focalDevice` | todo | | |
| Non-implantable Device uses (assessment scales, software) | other Devices | todo | | |

## Goals to Goal

See `docs/decisions/0014-goals.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Goal Observation code / value | `description` | done | `description_is_the_code_or_the_value_for_assertion` | |
| `statusCode` | `lifecycleStatus` | done | `status_map` | |
| `effectiveTime` | `startDate`, `target.dueDate` | done | `dates_become_start_and_target` | |
| `author` | `expressedBy`, Provenance | done | | |
| Progress Toward Goal (4.110) | `achievementStatus` | todo | | |
| Goal target measures, priority | `target.measure`, `priority` | todo | | |
| Links to Care Plan, Health Concerns | `addresses` | todo | | |

## Plan of Treatment to CarePlan

See `docs/decisions/0015-care-plan.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Plan of Treatment section | `CarePlan` (intent plan, category assess-plan) | done | `plan_has_required_fields_and_escaped_narrative` | |
| Section narrative | `text.div` | done | same | |
| Goals in the document | `goal` | done | `plan_points_at_goals` | |
| Planned activities (4.39-4.41, 4.44) | ServiceRequest | done | see below | |
| Planned activities as `CarePlan.activity` | `activity.reference` | todo | | |
| Health Concerns | `addresses` | todo | | |
| Care team | `careTeam`, `contributor` | todo | | |

## Care Team to CareTeam

See `docs/decisions/0016-care-team.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Care Team Organizer | `CareTeam` | done | `member_becomes_participant_with_role_and_period` | |
| `statusCode` | `status` | done | `status_map_and_fallback` | |
| Member performer | `participant.member` (Practitioner) | done | same as first | |
| Performer code | `participant.role` | done | same | |
| Member effectiveTime | `participant.period` | done | same | |
| `functionCode`, patient/related-person members | `role`, RelatedPerson | todo | | |
| PractitionerRole, Location | PractitionerRole, Location | todo | | |

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
| Subscriber as person, plan name | `subscriber`, `class` (plan) | todo | | |
| Authorization (4.123) | ServiceRequest / ClaimResponse | todo | | |

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
| Specimen Condition Observation | `condition` | todo | | |
| Location status, organization | `status`, `managingOrganization` | todo | | |

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
| Planned medication / supply | MedicationRequest / SupplyRequest | todo | | |
| Instructions (4.20) | `patientInstruction`, Communication | todo | | |

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
| Resources without an identifier | `section.entry` | todo | | |
| `legalAuthenticator`, `authenticator` | `attester` | todo | | |
| `componentOf/encompassingEncounter` | `encounter` | todo | | |
| `documentationOf/serviceEvent` | `event` | todo | | |
| `relatedDocument` | `relatesTo` | todo | | |

## Health Concerns

See `docs/decisions/0022-health-concerns.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Problem Concern Act in the Health Concerns section | `Condition` (category health-concern) | done | `category_follows_the_section_code` | |
| Health Concern Act (4.132) pointing by Entry Reference (4.122) | adds category `health-concern` | done | `a_pointed_at_condition_gains_the_health_concern_category_once` | |
| Problem Observation inside a Health Concern Act | `Condition` (health-concern) | done | `a_wrapped_problem_observation_becomes_a_health_concern_condition` | |
| Unresolved pointer, empty concern | report entries | done | `an_unresolved_pointer_is_reported` | |
| Pointer at allergy, result, social history | category / `addresses` | todo | | |
| Risk Concern Act | Condition / RiskAssessment | todo | | |

## Functional, cognitive, mental and health status

See `docs/decisions/0023-status-assessments.md`.

| C-CDA | FHIR | Status | Test | Validated |
|---|---|---|---|---|
| Functional Status Organizer (4.66) | panel `Observation` + `hasMember` | done | `organizer_is_a_panel_with_members_and_the_functional_status_category` | |
| Functional Status / Problem Observation, Self-Care Activities | `Observation` (functional-status) | done | same | |
| Cognitive Status Organizer / Observation (4.75 / 4.74) | `Observation` (cognitive-status) | done | `cognitive_and_mental_and_health_status_categories` | |
| Mental Status Organizer / Observation / Problem | `Observation` (cognitive-status, assumed) | done | same | |
| Health Status Observation (4.5) | `Observation` (survey) | done | same | |
| Assessment Scale Observation (4.69) | `Observation` + components | todo | | |
| Caregiver Characteristics (4.72), Disability Status | `Observation` | todo | | |

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

## Running real-world samples

`cargo run --release -p ccda2fhir --example corpus -- <files>` converts a set of documents and counts what was not mapped (report
code `entry.unmapped`, with the template OID). On the 29 sample documents in the Medplum repository (ONC 170.315(b)(1) samples and
small unit samples) nothing fails to convert and no entry is unmapped. Gaps it found earlier are now covered: Health Concerns,
status assessments, planned medications, instructions and procedure observations.

## Open policy questions

- **Display names.** We pass `displayName` through unchanged. Real documents often carry displays that
  differ from the code system's official display, and the validator reports those as errors on our
  output (found with a deliberately realistic Immunization fixture). Options for later, as configuration:
  keep (lossless, current), drop when the system is known, or replace from a terminology source.
- **Prior art on codes** (read from source): Medplum and Microsoft both pass `displayName` through
  unchanged. Medplum uses `displayName` or the raw `#ref` string as `text`; Microsoft emits no `text`
  and drops most translations. Microsoft also maps a value-set OID mistakenly used as a code system to
  SNOMED, a robustness hack worth considering as an opt-in repair.
- **OID table.** Entries are added as fixtures need them (NCI Thesaurus, v3 ActReason so far); the table
  still needs a full review against the HL7 OID registry.
