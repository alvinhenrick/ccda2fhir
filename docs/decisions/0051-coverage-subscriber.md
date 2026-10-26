# 0051: Coverage subscriber

## Status
Accepted

## Context
ADR 0017 mapped the policy holder's id to `subscriberId` only. `Coverage.subscriber` is a
reference to the person who holds the policy.

## Decision
- When the covered party's role is SELF, `subscriber` points at the Patient.
- Otherwise, when the Policy Holder Role (4.90) has a name, it becomes a `RelatedPerson` (name,
  identifier, telecom, address) for the patient, and `subscriber` points at it.
- A policy holder with no name gets no `subscriber`; `subscriberId` is still set. Nothing is
  invented. The RelatedPerson has no `relationship`, because the document gives the dependent's
  role, not the subscriber's.

## Consequences
- One RelatedPerson per policy; they are not merged across policies.
