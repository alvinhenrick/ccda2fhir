# 0033. Generated narrative

Status: accepted

## Decision

Resources without `text` get a generated one: a table of a fixed list of fields already present in the resource (name, code, status, dates, value). Text is XHTML-escaped and `text.status` is `generated`. Existing narrative is never replaced. Bundle, Binary and Parameters are skipped.

## Consequences

Deterministic output, no new facts, and the `dom-6` warnings go away. A fuller per-resource narrative can replace the generic table later.
