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
| Planned procedure (mood INT) | ServiceRequest | reported, todo | | |

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
