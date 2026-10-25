# 0037. Command line tool and HTTP service

Status: accepted

## Decision

- One binary, `ccda2fhir`, with `convert`, `serve` and `healthcheck`. The library stays pure; all I/O lives in the `ccda2fhir-server` crate.
- axum on tokio. Conversion runs in `spawn_blocking`. Limits (body size, concurrency, timeout) have safe defaults and are set by environment variables so a fork can deploy with no code change.
- Request bodies are never logged. Errors are `OperationOutcome`s built from the library's content-free messages.
- The image is multi-stage, on distroless `cc` as a non-root user. `healthcheck` is a subcommand because the image has no shell or curl.
- No authentication yet. The README says so; OIDC/JWT, TLS and RBAC come with the security layer.
- A `Docker` workflow builds the image, converts a fixture through it and checks the container health, because the image cannot be built in every development environment.

## Consequences

Forks can run `docker compose up` and have a working converter. Authentication must be added in front until the security layer lands.

## Update (docs refresh)

The security layer (authentication, TLS, authorization) is still to come; run the service behind your own gateway until then.
