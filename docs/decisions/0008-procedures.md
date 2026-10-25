# 0008: Procedures to Procedure

Status: accepted. Source: IG page "CF-procedures", CSV and ConceptMap `CF-ProcedureStatus`.

- The Procedure and Act variants map to `Procedure` (the Act is a subset). The Observation variant is an observation and is
  reported. Mood INT is a ServiceRequest and is reported until that mapper exists.
- Status map: completed, active to in-progress, aborted to stopped, cancelled to not-done. Not in the IG map, so ours:
  nullified to entered-in-error, held/suspended to on-hold, new to preparation. `negationInd` always gives not-done.
- Time: a point is `performedDateTime`; an interval is `performedPeriod`; no time is a data-absent reason, as the IG says.
- The author is `recorder` (with Provenance). Performers are `performer.actor`; the IG's PractitionerRole is deferred
  until that resource is mapped.
- `us-core-procedure` is claimed when code and a performed element are present.
