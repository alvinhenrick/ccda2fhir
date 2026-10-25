# 0004: Authors, Practitioner, Organization and Provenance

Status: accepted. Validated by the CI validator on the problems and allergies fixtures.

## Context
The IG states it "does not provide definitive guidance" on attributes (`recorder`) versus
`Provenance` resources, and does not address author context conduction (mappingGuidance, "CDA <-> FHIR
Provenance"). The problem and allergy pages do say: only the **last** author maps to `recorder`, only the
**earliest** `author/time` maps to `recordedDate`, and authors recorded only on the concern act should be
mapped when the observation has none.

## Decisions
- `recorder` = the latest author (by time; later in the document breaks ties). `recordedDate` = the
  earliest author time. Observation authors win; concern authors are the fallback.
- Each distinct person is one `Practitioner`, keyed by the first identifier (root + extension), else by
  name. Each distinct organization is one `Organization`, keyed the same way. Authors that are software
  (`assignedAuthoringDevice`) are reported, not mapped.
- Every target with at least one person author also gets a US Core `Provenance`: `target`, `recorded`,
  and one `agent` per distinct author with `type = author`, `who`, and `onBehalfOf` when there is an
  organization.
- `Provenance.recorded` is an `instant` (time and offset required). If the earliest author time has no
  time/offset, no Provenance is created and `provenance.recorded.unavailable` is reported;
  `recordedDate` is still set. We do not invent a time.
- A profile is declared only when the resource meets it: `Practitioner` claims US Core only with an
  identifier (system and value) and a family name. `Organization` claims none (its required `active` is
  not in C-CDA).
- References between resources are `urn:uuid:` values from `Context::reference`, identical to the entries'
  `fullUrl`.

## Not done
`PractitionerRole` (specialty from `assignedAuthor/code`), author telecom/address, and Provenance for
non-author participants (informants, performers). Tracked in docs/coverage.md.
