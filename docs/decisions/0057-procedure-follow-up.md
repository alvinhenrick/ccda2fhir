# 0057: Procedure follow-up

## Status
Accepted

## Context
An Instruction (4.20) nested in a procedure tells the patient what to do afterwards. The IG's
procedure page does not cover it; ADR 0024 only dealt with top-level instructions, and one in a
medication goes to the dosage.

## Decision
A nested Instruction becomes an entry of `Procedure.followUp`: its code (if any) as the coding,
its text as `text`. Identical entries are not repeated. This follows the meaning of the element
("specific follow up required", for example removing sutures), not an IG row.

## Consequences
- The instruction is no longer silently dropped.
