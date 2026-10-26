# 67. Optional JWT authentication for the service

Date: 2026-10-26. Status: accepted.

## Decision

- `POST /convert` can require a bearer JWT. Authentication is on only when `AUTH_JWKS_URL` or
  `AUTH_JWKS` is set; `/healthz` and `/readyz` are always open. The library is untouched: auth is
  a middleware in the service crate.
- The service **verifies** tokens, never issues them, and holds no secret. Keys come from the
  identity provider's JWKS, by URL (fetched at start, refreshed when older than the TTL, and on an
  unknown `kid`) or inline.
- Only `RS256` and `ES256` exist. There is no code for `none` or `HS*`, so those attacks have
  nothing to exploit, and the token's `alg` must be on the configured list and match the key type.
- Checks: signature, `exp` (required), `nbf`, `iss`, `aud` (string or list), and an optional scope
  (`scope` string or `scp`). Clock skew is configurable (60 s default). 401 for anything wrong with
  the token, 403 only for a genuine token without the scope. Responses are `OperationOutcome`s with
  an RFC 6750 `WWW-Authenticate`; a token is never logged or echoed.
- Misconfiguration fails at start: issuer or audience without a key set, both key sources, plain
  `http://` for a non-loopback URL, an empty key set.

## Why this shape

- **`ring`, not `jsonwebtoken`.** `jsonwebtoken` with its RustCrypto backend pulls `rsa`
  (RUSTSEC-2023-0071, no fix) and an old `crypto-bigint` (RUSTSEC-2026-0387). Both concern secret
  key operations, which a verifier never performs, but a clean `cargo deny` with no ignores is worth
  about 150 lines of code. `ring` is already in the tree through rustls, and its verification API
  is exactly what a JWKS needs. The licence list gains ISC and CDLA-Permissive-2.0 (the root-store
  data).
- **Refresh is rate limited.** A token naming an unknown `kid` causes at most one key fetch per
  `AUTH_JWKS_MIN_REFRESH_SECS`, failures included, so junk tokens cannot make the service hammer
  the provider. Fetches have a 5 s timeout, a 1 MiB cap and follow no redirects.
- **The one outbound request.** ADR 0065 says the service makes no outbound requests. The JWKS
  fetch is now the single exception, only when `AUTH_JWKS_URL` is set, and only to that URL.

## Not done

Token introspection, opaque tokens, mutual TLS, per-client rate limits and audit logs belong to a
gateway or a later layer. `PS256`, `RS384`, `EdDSA` can be added if a provider needs them.
