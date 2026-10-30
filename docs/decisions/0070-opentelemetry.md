# 0070: OpenTelemetry traces from the service

Status: accepted

## Context

Teams that run the service want its requests in the tracing system they already have. The converter itself is a
pure library and must stay that way.

## Decision

- Only the service crate changes. The libraries do not depend on OpenTelemetry.
- It is a cargo feature, `otel`, off by default so the CLI and the library stay small. The container image builds
  with it.
- Export is switched on by the standard `OTEL_EXPORTER_OTLP_ENDPOINT` (or `..._TRACES_ENDPOINT`). With it unset, no
  exporter starts and nothing is sent. All other settings are the standard `OTEL_*` variables, read by the SDK.
- Transport is OTLP over HTTP with protobuf, which every collector accepts and which needs no gRPC stack.
- The request span (INFO level, so it passes the default filter) carries method, path, status and latency. A
  `converted` event carries the status word, the issue count and the byte size. Nothing from the document, no
  headers and no tokens are recorded, in line with the rule that request bodies are never logged.
- If the exporter cannot start, the service logs a warning and runs without it. A telemetry fault must not stop
  clinical data conversion.
- A binary built without the feature warns when the variable is set, so a missing feature is not silent.

## Consequences

- No metrics yet. Counts per issue code would be the natural next step if asked for.
- The `otel` build pulls in reqwest, prost and the OpenTelemetry SDK. They are covered by the licence and advisory
  checks like everything else.
