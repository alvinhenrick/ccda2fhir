# 0060: CarePlan addresses

## Status
Accepted

## Context
`CarePlan.addresses` lists the Conditions a plan is for. The C-CDA does not link a Plan of
Treatment section to the problems or health concerns around it.

## Decision
When a document has a care plan and Conditions that are health concerns (from Health Concern Acts),
the plan's `addresses` lists those Conditions. This is a judgment, made the same way every goal in
the document is attached to the plan, and it is reported (`careplan.addresses.inferred`). Problem
list Conditions are not added. A plan that already has `addresses` is left alone.

## Consequences
- Documents with a plan but no health concerns get no `addresses`.
