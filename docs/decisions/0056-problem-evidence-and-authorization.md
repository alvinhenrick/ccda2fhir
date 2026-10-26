# 0056: Problem evidence; authorization left unmapped

## Status
Accepted

## Context
Supporting observations (SPRT) on a problem were reported, not mapped. The Authorization Activity
(4.123) was reported as unmapped.

## Decision
- Each Observation nested in a Problem Observation with `typeCode="SPRT"` becomes an `Observation`
  (the common observation mapper, with its author) and a `Condition.evidence.detail` reference, as
  the IG directs. If the same observation also appears elsewhere in the document with the same id,
  two Observations result; they are not merged.
- The Authorization Activity has no mapping in the IG, and no R4 resource fits it without guessing
  (it is a permission for planned care, not a claim or an eligibility check). It stays unmapped and is reported when it sits on a policy
  (`coverage.authorization.unsupported`) rather than being given a wrong shape. The same holds for a
  standalone supply request.

## Consequences
- `condition.evidence.unsupported` is gone.
