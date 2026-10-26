# 0062: Coverage plan, member period and Observation reasons

## Status
Accepted

## Context
The Policy Activity carries the plan in a nested act (REFR, mood DEF) with an id and the plan name
in `text`, and the covered party's `time` is the member's coverage period. ADR 0050 linked an
Indication only to Conditions.

## Decision
- The plan act becomes a `Coverage.class` entry of type `plan`: `value` is its id (else the name),
  `name` is the text.
- The covered party's `time` becomes `Coverage.period` when the policy has no interval of its own.
- An Indication whose id matches an Observation (a result, for example) now also gives
  `reasonReference`. A Condition with the same id is preferred. All five resource types allow an
  Observation there.

## Consequences
- Only the Authorization Activity is left unmapped in the Policy Activity (ADR 0056).
