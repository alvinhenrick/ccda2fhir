# 0038. PractitionerRole

Status: accepted

## Decision

An author, performer or participant that has a role or specialty code, or an organization, also gets a `PractitionerRole` linking the Practitioner, the Organization and the code. One role is created per practitioner, code and organization, however often the person appears. A NUCC taxonomy code (2.16.840.1.113883.6.101) is the `specialty`; any other role code is `code`. References from clinical resources still point at the Practitioner, so existing links do not change.

## Consequences

The author role code, which the data-loss audit showed as the largest single loss in the sample documents (about 58 values), is kept. The audit tool now names the template that drops each value and ignores two known false positives (the telecom scheme prefix and the gender code system).
