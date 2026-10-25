# Coverage matrix

Status per C-CDA element: **done** (mapped and tested), **partial**, **reported** (not mapped; the
report says so), **todo**. "Validated" stays empty until the HL7 validator has run on the output.
Source of each rule: the IG page and CSV under `ccda-on-fhir` (`CF-*.md`, `mappings/CF/*.csv`).

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
| Author | `recorder`, `recordedDate`, Provenance | todo (needs author model) | | |
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
| Author | `recorder`, `recordedDate`, Provenance | todo (author model) | | |
| Comment Activity | `note` | todo | | |
