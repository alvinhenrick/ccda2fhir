# Security

## Reporting a vulnerability
Please report suspected vulnerabilities privately through GitHub's "Report a vulnerability" (Security tab) rather
than a public issue. Do not include real patient data in a report.

## What the library does today
- Rejects any `DOCTYPE`, so entity-expansion attacks do not apply.
- Enforces input size and XML node limits (configurable through `Limits`).
- Contains no `unsafe` code (forbidden at workspace level).
- Keeps document content out of error and report messages.
- Holds no state and does no I/O.

## What it deliberately does not do yet
Authentication, authorization, TLS, rate limiting and audit export belong to the service layer, which does not
exist yet. The plan is pluggable seams (`Authenticator`, `Authorizer`, `AuditSink`) with permissive defaults, an
OIDC/JWT validator, and TLS through rustls. Until then:

- Do not expose any wrapper around this library to untrusted networks.
- Do not use it with real patient data in an unauthenticated deployment.

This project has not had an independent security review.
