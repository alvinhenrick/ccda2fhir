# 0036. SDOH category

Status: accepted

## Decision

C-CDA does not flag social determinants of health. A resource gets the US Core `sdoh` category (added to its existing categories) only when its code is on a fixed list: ICD-10-CM Z55 to Z65 for Conditions, and the LOINC screening tools PRAPARE (93025-5), AHC-HRSN (96777-8), Hunger Vital Sign (88121-9, 88122-7, 88123-5) and housing status (71802-3) for Observations. Each tag is reported as `sdoh.category.derived`. Codes not on the list are never guessed.

## Consequences

SDOH problems and screening results are findable by category. The lists are short on purpose and can grow; each addition needs a source.
