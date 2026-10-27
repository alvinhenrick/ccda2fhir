# Security and deployment

## Data

Use **synthetic or de-identified** documents until you have done your own security and compliance review. The
project has not had an independent security review (see [Security](../../SECURITY.md)). Fixtures in the repository are
synthetic. Request bodies, tokens and document values are never logged and never appear in error or report messages.

## Running the service

The service is a plain HTTP server. Put it behind something that terminates **TLS** (a reverse proxy, a gateway, a
cloud load balancer) and enable authentication (above) unless the network already guarantees who can call it. It
has no rate limiting per client and no audit log; add those at the gateway.

The Docker image is small, runs read-only with all capabilities dropped (`docker-compose.yml`), and has a health check:

```
docker compose up --build
```

Sensible production settings:

- `MAX_BODY_BYTES` as small as your largest real document needs (default 10 MiB).
- `MAX_CONCURRENCY` about the number of CPU cores to a few times that: conversion is CPU work.
- `REQUEST_TIMEOUT_SECS` just above your slowest conversion.
- `DISABLE_UI=1` if you want an API only.
- `AUTH_JWKS_URL`, `AUTH_ISSUER`, `AUTH_AUDIENCE` and `AUTH_REQUIRED_SCOPE` so only your own clients can convert.

## The browser module

The WebAssembly build (`scripts/build-wasm.sh web`) runs the same converter in the page, so documents never leave the
machine. Serve it with a Content-Security-Policy that allows `'wasm-unsafe-eval'` for your own scripts only; the
hosted demo shows the exact policy in its page source.

## Checking your own documents

An opt-in test rig converts every `.xml` in a folder through the service and validates the bundles with the official
HL7 validator against FHIR R4 and US Core 6.1.0:

```
INPUT_DIR=./my-documents docker compose --profile validate up --build \
  --abort-on-container-exit --exit-code-from validator
docker compose --profile validate down -v
```

The validator checks that the output is valid FHIR. It cannot tell whether every clinical value is mapped as you
would want, so have a clinician or integrator review the output before it is used for care.
