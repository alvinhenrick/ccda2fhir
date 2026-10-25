# 0022: Health Concerns

Status: accepted. The IG's Problems page covers the `health-concern` category; the Health Concern Act (4.132) itself has no page, so
this follows C-CDA and US Core Condition Problems and Health Concerns.

- A Problem Concern Act (4.3) in the Health Concerns section (LOINC 75310-3) already becomes a Condition with category
  `health-concern`, by the section rule in the problem mapper.
- A Health Concern Act (4.132) usually does not hold data itself. It points at entries elsewhere with Entry Reference acts
  (4.122), whose id equals the id of a Problem Observation. The pointed-at Condition gains the `health-concern` category next to
  its own (once, even if pointed at twice). A pointer that matches no Condition is reported (`healthconcern.reference.unresolved`).
- A Problem Observation nested directly in the concern becomes a Condition with category `health-concern`.
- Anything else inside the concern, or an empty concern, is reported (`healthconcern.nested.unsupported`, `healthconcern.empty`).
- The concern's own code (75310-3) and statusCode are not mapped: they describe the wrapper, not a condition.
- Not mapped yet: pointers at non-Condition entries (allergies, results, social history), and the Risk Concern Act.
