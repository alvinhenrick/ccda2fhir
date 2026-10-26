# 0061: Result categories from a LOINC table

## Status
Accepted

## Context
The IG sets a result's category from the LOINC `CLASSTYPE` of its code (1 laboratory, 2 clinical,
3 claims attachments, 4 survey). LOINC is licensed separately, so the library cannot bundle it.
Until now every result was treated as laboratory and the assumption was reported.

## Decision
- `ccda-terminology` has a `LoincTable`, read from the `LOINC_NUM` and `CLASSTYPE` columns of the
  caller's own `Loinc.csv`. `Options::loinc` carries it; the CLI and service take
  `--loinc-table` (or `LOINC_TABLE`). A table that cannot be read or lacks those columns stops
  start-up; it is not silently ignored.
- With a table: class 1 gives `laboratory` (and report category `LAB`), class 4 gives `survey`.
  Classes 2 and 3 have no R4 category code (the IG's `clinical-test` is not in R4's value set), so
  no category is set and `results.category.unavailable` is reported.
- A code the table does not know, or a non-LOINC code, and every run without a table, keep the
  old behavior: laboratory, reported as `results.category.assumed-laboratory`.

## Consequences
- Output without a table is unchanged.
- The Observation profile choice follows the category, so a clinical-class result no longer claims
  the US Core laboratory profile.
