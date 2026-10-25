# 0017: Insurance (Policy Activity) to Coverage

Status: accepted. No IG page covers insurance; this follows C-CDA Payers section, Coverage Activity (4.60) / Policy Activity
(4.61), and the US Core Coverage profile.

- Each Policy Activity (found anywhere in the entries) becomes a `Coverage` with `beneficiary` the patient and `type` from
  the policy code (Source of Payment Typology, `https://nahdo.org/sopt`).
- `status` is `active` unless the policy was nullified (`entered-in-error`): the document cannot say a policy lapsed, and
  `completed` on an activity means the activity was recorded, not that coverage ended.
- Covered Party (4.89): participant role id is the member id (`identifier`, type MB) and its code gives `relationship`
  (SELF→self, spouse, child, parent, otherwise other and reported). Policy Holder (4.90): id gives `subscriberId`.
- The policy's own ids are the group number (`class`, type group). `period` is the policy effectiveTime.
- `payor` is the Payer performer's represented organization (shared Organization). A guarantor performer is ignored. A
  missing payer is reported (`coverage.payor.missing`).
- `us-core-coverage` is claimed only with payor, relationship and subscriberId.
- Not mapped yet: subscriber as RelatedPerson, plan name, authorizations, participant `time` as the member's period.
