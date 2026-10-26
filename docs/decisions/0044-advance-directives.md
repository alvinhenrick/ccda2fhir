# 0044. Advance directives as Consent

Status: accepted

## Decision
- An Advance Directive Observation (4.48), alone or inside an Organizer (4.108), becomes a
  `Consent` with scope `adr`. The directive type (`code`) is the category; the coded value
  (for example do not resuscitate) is `provision.code`; the effective time gives `dateTime` and
  `provision.period`.
- The provision has no `type`: the document does not say permit or deny, and we do not guess.
- Status: completed or active gives `active`; aborted, cancelled or suspended gives `inactive`;
  nullified gives `entered-in-error`; anything else is `active` and reported
  (`consent.status.assumed`).
- A missing type code uses the generic advance directive LOINC 42348-3 and is reported.
- Verifier and custodian participants are mapped in ADR 0059. Any other participant is reported
  once per directive (`consent.participants.unsupported`).
- R4 requires `policy` or `policyRule` (ppc-1). The document names none, so `policyRule` carries
  the data-absent-reason extension (unknown); no rule is invented.
- No US Core profile is claimed (US Core 6.1.0 has none for Consent).
