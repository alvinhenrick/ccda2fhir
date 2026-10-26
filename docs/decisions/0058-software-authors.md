# 0058: Software authors

## Status
Accepted

## Context
An author that is software (`assignedAuthoringDevice`) was reported and dropped, so the
Provenance did not say that a system created the data. The Composition already used a Device.

## Decision
A software author becomes a `Device` (shared by id and name) and an agent of the resource's
Provenance (type author; `onBehalfOf` the represented organization if any). A Device cannot be a
`recorder`, so `recorder` still comes from the latest author who is a person. Provenance is made
when any author, person or software, gives a time with an offset. `author.device.unsupported` is
gone.

## Consequences
- A resource authored only by software has no `recorder` but does have a Provenance.
