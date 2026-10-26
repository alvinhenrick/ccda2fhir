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

## What the service does today
- Optional JWT bearer authentication on `POST /convert` (RS256/ES256, JWKS by URL or inline, issuer, audience,
  expiry and optional scope). See ADR 0067 and the README. It is off unless you configure a key set.
- Caps body size, concurrency and request time; never logs request bodies or tokens.

## What it deliberately does not do yet
Authorization beyond one required scope, TLS termination, rate limiting per client and audit export belong to your
gateway or a later layer. Until then:

- Run the service behind TLS (a reverse proxy or gateway) and turn authentication on.
- Do not expose it to untrusted networks without authentication.
- Do not use it with real patient data in an unauthenticated deployment.

This project has not had an independent security review.
