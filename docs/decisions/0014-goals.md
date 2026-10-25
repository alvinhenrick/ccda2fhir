# 0014: Goals to Goal

Status: accepted. No IG page covers goals; this follows C-CDA Goal Observation (4.121) and the US Core Goal profile.

- Every Goal Observation becomes a `Goal`. `description` is the observation code, or the value when the code is the
  generic ASSERTION. A goal with neither is still emitted and reported (`goal.description.missing`); it does not claim
  `us-core-goal`.
- `lifecycleStatus` from statusCode: new→proposed, active→active, completed→completed, suspended/held→on-hold,
  aborted/cancelled→cancelled, nullified→entered-in-error. Missing or unknown becomes `active` and is reported.
- `effectiveTime` low (or value) becomes `startDate`; high becomes `target.dueDate`. Both are dates, so time is dropped.
- The latest author becomes `expressedBy` (and a Provenance as elsewhere).
- Progress toward a goal (4.110) is reported (`goal.progress.unsupported`) until `achievementStatus` is mapped.

## Update (docs refresh)

Still open: progress toward a goal (`achievementStatus`), `priority` and `addresses`.
