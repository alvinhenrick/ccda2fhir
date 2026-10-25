# 0010: Social history to Observation

Status: accepted. Source: IG page "CF-social".

- Social History, Smoking Status MU and Tobacco Use are Observations with category `social-history`; status, value and
  detail rules are the shared ones (ADR 0006). A start-only time is `effectiveDateTime` (the IG prefers one date).
- Pregnancy: the code is LOINC 82810-3 (status, mood EVN) or 86645-9 (intent, mood INT) even when the document says
  ASSERTION. The one allowed time is the first populated of value, low, high, author time. An unknown value (nullFlavor
  UNK) becomes a v3 NullFlavor coding as the IG says. The estimated delivery date is component 11778-8.
- Profile claims: smoking status, pregnancy status and intent need a coded value and a single date; any other social
  observation with a value claims `us-core-simple-observation`.
- Birth sex and gender identity are Patient extensions and wait for the fuller Patient mapper.
