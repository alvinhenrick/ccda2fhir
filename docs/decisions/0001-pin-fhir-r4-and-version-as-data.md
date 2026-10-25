# 0001: Pin FHIR R4; treat target versions as data

Status: accepted (US Core pairing still to be verified)

## Context
US Core, and most certified health IT, is published against FHIR R4 (4.0.1). USCDI v1 and v3 correspond to
different US Core releases, and C-CDA templates differ between R1.1 and R2.1.

## Decision
- Output FHIR R4 (4.0.1). R4B and R5 are out of scope until someone needs them.
- Target US Core 3.1.1 for USCDI v1 and 6.1.0 for USCDI v3. **Verify this pairing against the US Core pages before locking.**
- Input is C-CDA R2.1. R1.1 is deferred.
- Version differences live in rule data keyed by target, not in scattered conditionals. One version-neutral internal model.
- FHIR knowledge stays inside `fhir-out` and the mappers, so another FHIR version can be added as an output adapter.

## Consequences
Adding a US Core version starts as a copy of a rule set plus a diff, with its own golden files and validator run.
Some version changes (renamed profiles, changed bindings) need real rework, not just a copied file.
