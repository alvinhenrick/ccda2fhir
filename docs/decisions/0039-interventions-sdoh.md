# 0039. Interventions and SDOH goals

Status: accepted

## Decision

- An Intervention Act (4.131) or Planned Intervention Act (4.146) only wraps the real activities. The mappers now look through the wrapper, so the Procedure Activity inside becomes a `Procedure` and the planned activity a `ServiceRequest`. Before this, these were reported as unmapped and the interventions were lost.
- The SDOH tag follows ADR 0036: ICD-10-CM Z55 to Z65 on a Procedure or ServiceRequest `code`, or on a Goal `description`. A tagged Procedure gets the SNOMED category "Social service procedure" (410606002) when it has none; ServiceRequest and Goal get the US Core `sdoh` category.

## Not done

Goals are tagged only by code. Linking a goal to the SDOH problem it addresses (`Goal.addresses`) is separate work.
