# 0045. Derivation Provenance

Status: accepted

## Context
Author `Provenance` (ADR 0004) says who wrote a fact. For exchange (TEFCA, audit) a consumer also
needs to know which source document a resource came from and what produced it.

## Decision
- One `Provenance` per converted document. Targets: the Patient and every clinical resource in
  the bundle. Activity: `Derivation` (w3c-provenance-activity-type). Entity: role `source`,
  pointing at the Composition, which stands for the source document.
- The R4 `Provenance.activity` value set (preferred binding) has no derivation code, so the activity
  carries two codings: `Derivation` and `CREATE` from v3-DataOperation. This keeps the HL7
  validator quiet and the meaning intact.
- Agent: type `assembler`, who is a `Device` named `ccda2fhir` with the crate version.
- `recorded` is the document's own time, never the wall clock, so output stays deterministic.
  If the document time has no offset it cannot be an `instant`; nothing is added and
  `provenance.derivation.unavailable` is reported.
- No US Core Provenance profile is claimed: it requires `onBehalfOf` for practitioner agents and
  this agent is a Device.
- Goldens replace the crate version with a placeholder, so a version bump does not touch them.

## Consequences
One extra Provenance and Device per bundle. A per-resource Provenance was rejected as noise.
