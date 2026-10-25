# 0048: Goal progress, priority and addresses

## Status
Accepted

## Context
ADR 0014 left three Goal elements unmapped: progress toward the goal (template 4.110), the
Priority Preference (4.143) and the link to the problem a goal addresses.

## Decision
- `achievementStatus` carries the value of the Progress Toward Goal observation as the document
  gives it: its code if coded, otherwise its text. The R4 achievement codes (`in-progress`,
  `improving`, ...) are not guessed from free text. An observation with no usable value is
  reported as `goal.progress.empty`.
- `priority` carries the coded value of the Priority Preference observation.
- `addresses` points at the Conditions named by Problem Observations nested in the goal. Matching
  is by identifier after the Conditions exist, like the encounter links (ADR 0025). A problem that
  is not in the document is reported as `goal.addresses.unresolved`; nothing is invented.

## Consequences
- The old `goal.progress.unsupported` report is gone.
- Care plan and health concern `addresses` links are still open.
