# 0015: Plan of Treatment to CarePlan

Status: accepted. No IG page covers care plans; this follows C-CDA Plan of Treatment (2.2.10) and the US Core CarePlan profile.

- Each Plan of Treatment section with narrative or goals becomes one `CarePlan`: `intent` plan, category `assess-plan`,
  `subject`, `title` from the section title, and `goal` pointing at every Goal in the document.
- `text` is the section narrative flattened to plain text and XML-escaped (US Core requires a narrative). With no
  narrative it says "See goals."
- The document does not carry a plan status. `active` is assumed and reported (`careplan.status.assumed`).
- Planned activities (planned medication, procedure, encounter, observation, supply, instructions) are not yet turned into
  `activity` entries or ServiceRequests; that comes with the ServiceRequest work.

## Update (docs refresh)

Done since: `CarePlan.activity` points at the requests and appointments of the Plan of Treatment section (ADR 0034); planned medications are MedicationRequests (ADR 0024).
