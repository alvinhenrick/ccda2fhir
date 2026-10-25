# 0023: Functional, cognitive, mental and health status

Status: accepted. No IG page covers these; this follows C-CDA and US Core (Simple Observation, with the `functional-status` and
`cognitive-status` categories of the US Core category code system).

- Organizers (Functional Status 4.66, Cognitive Status 4.75, Mental Status 4.285) become a panel `Observation` (its own code, status
  and time, or the span of its members' times) with `hasMember` links. Each observation component becomes its own `Observation`.
- Observations (Functional Status 4.67, Functional Status Problem 4.68, Self-Care Activities 4.128, Cognitive Status 4.74, Mental Status
  4.284, Mental Status Problem 4.125, Health Status 4.5), loose or in an organizer, map like any Observation (code, value, status,
  time, performer). A null-flavored code (as Self-Care Activities uses) becomes a data-absent reason.
- Category: functional-status, cognitive-status. Mental status has no US Core category and uses cognitive-status (reported:
  `assessment.category.assumed`). Health status has none either and uses the base `survey` category (reported).
- `us-core-simple-observation` is claimed for functional and cognitive status when the Observation has a coded `code`, a value and a time.
- Not mapped yet: Assessment Scale Observations (4.69) and their supporting observations, Caregiver Characteristics (4.72), Disability
  Status, and the Mental Status template OIDs 4.284 and 4.285 are taken from C-CDA 3.0 and have no sample in the test corpus yet.

## Update (docs refresh)

Done since: Assessment Scale Observations (ADR 0029) and Disability Status and Caregiver Characteristics (ADR 0034).
