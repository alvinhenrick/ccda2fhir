# 0059: Advance directive participants

## Status
Accepted

## Context
ADR 0044 reported the participants of an Advance Directive Observation instead of mapping them.

## Decision
- A verifier (`VRF`, the clinician who checked the directive) becomes `Consent.verification` with
  `verified` true and `verificationDate` from the participant time. R4 has no place for the
  clinician (`verifiedWith` is a Patient or RelatedPerson), so who verified it is dropped and
  reported (`consent.verifier.identity-dropped`).
- A custodian (`CST`, the healthcare agent) with a name becomes a `RelatedPerson` (relationship
  from the playing entity code, address and telecom) and a `Consent.provision.actor`, whose
  required `role` is the participant role code, or a data-absent-reason if there is none.
- Any other participant (a witness has no template in C-CDA), or a custodian with no name, is
  reported once per directive (`consent.participants.unsupported`).

## Consequences
- Witnesses stay unmapped; Consent R4 has no element for them.
