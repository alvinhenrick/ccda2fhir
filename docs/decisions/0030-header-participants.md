# 0030: Header participants

Status: accepted. Found by the data-loss audit (`docs/coverage.md`).

| C-CDA header | FHIR |
|---|---|
| `legalAuthenticator` | `Composition.attester` (mode `legal`, with time) |
| `authenticator` | `Composition.attester` (mode `professional`) |
| `participant/associatedEntity` (next of kin, emergency contact, ...) | `Patient.contact` (relationship: the RoleClass and the entity code) |
| `dataEnterer` | `Provenance` agent of the Composition, type `enterer` |
| `informant` | `Provenance` agent of the Composition, type `informant` |
| `relatedDocument` (RPLC, APND, XFRM, SIGN) | `Composition.relatesTo` (`replaces`, `appends`, `transforms`, `signs`, by identifier) |
| `telecom` / `addr` of authors and organizations | `Practitioner` and `Organization` `telecom` / `address` |

- Composition has no element for the data enterer or informants, so they are Provenance agents, like authors elsewhere. The
  Provenance needs the document date with an offset; without it a warning is reported (`provenance.recorded.unavailable`).
- An informant who is a `relatedEntity` (a relative or friend of the patient) becomes a `RelatedPerson`, not a Practitioner.
- A party that is software or an organization becomes a Device or Organization (ADR 0020).
- Still not mapped: `informationRecipient`, `inFulfillmentOf`, `authorization`. They have no Composition element.
