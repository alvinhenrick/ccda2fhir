# 0019: Planned activities to ServiceRequest

Status: accepted. No IG page covers them; this follows C-CDA Planned Procedure (4.41), Planned Encounter (4.40), Planned
Observation (4.44), Planned Act (4.39), the activity templates written with a non-event mood, and US Core ServiceRequest.

- Selected: a planned template whose mood is not EVN, or a Procedure/Act/Encounter Activity whose mood is not EVN. These used to be
  reported as "not mapped" by the procedure and encounter mappers; those reports are gone.
- `intent` from the mood: PRP→proposal; RQO, ARQ, APT→order; INT, PRMS (and no mood)→plan. This differs from medications
  (INT→order) because the IG sets that rule only for MedicationRequest; for everything else INT means "intends", which FHIR calls plan.
- `status` from statusCode: active, new→active; completed; aborted, cancelled→revoked; held, suspended→on-hold; nullified→
  entered-in-error. Missing or unknown becomes `active` and is reported (`servicerequest.status.assumed`).
- `category` is not in C-CDA. A broad SNOMED category is chosen from the kind of activity (procedure 71388002, encounter 308335008,
  observation 386053000) and reported (`servicerequest.category.assumed`). A planned act gets none.
- `code` from the statement code (data-absent reason when missing, reported), `occurrence[x]` from effectiveTime, `bodySite` from
  targetSiteCode, `reasonCode` from an Indication, `requester`/`authoredOn` from the author (with Provenance).
- `us-core-servicerequest` is claimed only with category, authoredOn and requester.
- Not mapped yet: planned medications and supplies (medication mapper), Instructions (4.20), performers, `Appointment` for
  scheduled visits, `CarePlan.activity` links to these requests.

## Update (docs refresh)

Done since: planned medications and Instructions (ADR 0024), `Appointment` for scheduled visits and `CarePlan.activity` links (ADR 0034), referral performers (ADR 0036 onward). Planned supplies and planned immunizations are still not mapped.
