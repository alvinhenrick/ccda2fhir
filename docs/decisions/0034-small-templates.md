# 0034. Small templates

Status: accepted

## Decision

- Comment Activity (4.64): becomes a `note` Annotation (text, author name, time) on the resource whose identifier matches the parent statement. Unmatched comments are reported (`comment.unattached`), never guessed.
- Risk Concern Act (4.136): handled like the Problem Concern Act, producing Conditions.
- Disability Status (4.505) and Caregiver Characteristics (4.72): `Observation` with the functional-status category.
- Planned Encounter with mood APT: `Appointment` (`start`/`end` only when the time is an instant, else `requestedPeriod`).
- CarePlan `activity`: the plan points at the requests and appointments listed in its Plan of Treatment section, found by entry ids. Plans and sections are paired in order; if the counts differ nothing is linked and `careplan.activity.unpaired` is reported.

## Consequences

Golden `small_templates` and the two changed plan goldens pass the HL7 validator.
