# 0016: Care Team to CareTeam

Status: accepted. No IG page covers care teams; this follows C-CDA Care Team Organizer (4.500) and the US Core CareTeam profile.

- Each Care Team Organizer becomes a `CareTeam`: `status` from statusCode (completed/aborted/cancelled→inactive,
  held/suspended→suspended, new→proposed, nullified→entered-in-error), `subject`, and `period` from effectiveTime.
- Each Care Team Member Act performer becomes a `participant`: `member` is a Practitioner (shared with authors, so one
  person is one resource), `role` is the performer's code, `onBehalfOf` the represented organization, `period` from the member
  act's effectiveTime. A member with no name or id is skipped and reported.
- A missing or unknown status becomes `active` and is reported (`careteam.status.assumed`).
- `us-core-careteam` is claimed only when every participant has a role and a member.
- Patients and related persons as team members, and `functionCode`, are not mapped yet.
