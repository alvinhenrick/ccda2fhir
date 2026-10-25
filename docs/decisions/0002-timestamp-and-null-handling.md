# 0002: Timestamp, nullFlavor, identifier and coded-value rules

Status: accepted. Items marked "verify" need the HL7 validator harness.

## Context
Every resource mapper needs the same four building blocks. The IG's general mapping guidance
(`mappingGuidance.md`, sections "CDA id to FHIR identifier", "CDA <-> FHIR Time/Dates", "CDA Coding to FHIR
CodeableConcept", "Mapping OID <-> URI") and its ConceptMaps are the source of truth. Where the IG is silent or
offers options, the choice is recorded here.

## Decisions

**Timestamps (`ccda-map/src/timestamp.rs`)**
- Strict parse of the HL7 grammar `YYYY[MM[DD[HH[MM[SS[.S+]]]]]][+/-ZZ[zz]]`. Anything else (ISO-style `1970-01-02`,
  odd lengths, impossible dates including non-leap Feb 29, bad offsets) is invalid and reported, never half-converted.
- Precision is preserved. `20230531` becomes `2023-05-31`, not a datetime.
- An offset is preserved when present. A time **without** an offset is never given an invented one: the time is dropped,
  the date is kept, and a warning (`timestamp.time-dropped.no-offset`) is recorded. This is the first option the IG lists.
  The other options (borrow an offset from the same entry or document, assume the source's location, default to 00:00)
  can become configuration later.
- Missing minutes or seconds are filled with zeros and noted (`timestamp.precision-extended`, info). **Verify:** the IG's
  own example shows `2023-05-31T22:05-05:00`, but FHIR R4 `dateTime` requires `hh:mm:ss`, so we believe that example is
  not valid R4. Confirmed: the validator accepts our `:00`-filled output; we have not tested the IG's seconds-less form.

**nullFlavor (`nullflavor.rs`)**
- The table is the IG ConceptMap `CF-NullFlavorDataAbsentReason`. `INV`, `DER`, `UNC` and anything else not in that map
  become `unknown` with a warning (`nullflavor.unmapped`).

**Identifiers (`identifier.rs`)**
- One unit test per row of the IG's table. Known identifier systems (SSN, NPI) never get their OID sent as a value;
  the value is marked absent with a data-absent-reason instead. UUIDs are lowercased.

**Coded values (`coding.rs`)**
- Known OIDs map to their canonical URI and **never** to `urn:oid:` (the IG is explicit about LOINC). Unknown OIDs use
  `urn:oid:`. Translations become extra codings. `originalText` becomes `text` when it is plain text; references into the
  section narrative are reported until narrative handling exists.

## Consequences
- All reports use stable codes and never contain document values.
- The OID table in `ccda-terminology` still needs a full review against the HL7 registry.

## Update (docs refresh)

References into the section narrative are resolved when the narrative has the referenced element; one that cannot be found is reported as `coding.original-text.reference-unresolved`.
