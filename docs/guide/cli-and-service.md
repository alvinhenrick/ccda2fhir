# The command line and the HTTP service

One binary, `ccda2fhir`, with three commands.

## `convert`

```
ccda2fhir convert ccd.xml > bundle.json
ccda2fhir convert *.xml --out-dir out/          # writes out/<name>.fhir.json
```

| Flag | Meaning |
|---|---|
| `-o`, `--out-dir DIR` | Write `<name>.fhir.json` into `DIR` instead of next to the input. With one file and no `--out-dir` the bundle goes to standard output. |
| `--profiles us-core\|none` | Declare US Core 6.1.0 profiles (default) or none. |
| `--loinc-table FILE` | A `Loinc.csv` from your own LOINC download, used to set result categories. Also `LOINC_TABLE`. |

## `serve`

```
ccda2fhir serve --bind 127.0.0.1:8080
```

Every setting can be a flag or an environment variable:

| Flag | Variable | Default | Meaning |
|---|---|---|---|
| `--bind` | `BIND_ADDR` | `127.0.0.1:8080` (`0.0.0.0:8080` in the Docker image) | Address to listen on. |
| `--max-body-bytes` | `MAX_BODY_BYTES` | 10 MiB | Largest request body. Bigger is a 413. |
| `--max-concurrency` | `MAX_CONCURRENCY` | 64 | Conversions at once; further requests wait. |
| `--request-timeout-secs` | `REQUEST_TIMEOUT_SECS` | 30 | Time allowed per request. |
| `--loinc-table` | `LOINC_TABLE` | none | As above. |
| `--no-ui` | `DISABLE_UI` | off | Do not serve the web page. |
| | `RUST_LOG` | `info` | Log level. Request bodies are never logged. |

### `POST /convert`

Send the document as the request body:

| Content-Type | Body |
|---|---|
| `application/xml`, `text/xml`, `*+xml`, or none | The C-CDA XML. |
| `application/json`, `application/fhir+json` | `{ "document": "<base64 C-CDA>" }`, or a FHIR `DocumentReference` whose XML attachment has inline base64 `data`. A `url` attachment is not fetched. |

Query parameters: `report=true` returns `{ status, issues: { total, warnings }, bundle, report: [...] }` instead of the
bare bundle; `profiles=none` declares no profiles.

Every successful response carries `X-Ccda2fhir-Status` (`complete` or `partial`) and `X-Ccda2fhir-Issues` (the number
of report entries).

| Status | When |
|---|---|
| 200 | Converted. |
| 400 | Not a parseable C-CDA document, or invalid JSON or base64 input. The message never echoes the request. |
| 401, 403 | Authentication is on and the token is missing, invalid, or lacks the required scope. |
| 408 | The request took longer than the timeout. |
| 413 | The body is larger than `MAX_BODY_BYTES`. |
| 415 | An unsupported Content-Type. |

Errors are FHIR `OperationOutcome` resources.

### Other endpoints

`GET /healthz` and `GET /readyz` answer `ok`, for orchestrators; they are never authenticated. `GET /` is the web
page (a paste-or-drop converter with resource, report and JSON views). `ccda2fhir healthcheck` exits 0 if a local
service answers `/healthz`; the Docker image uses it.

### Authentication

Off by default. To turn it on, give the service your identity provider's keys and say who a token must be for:

| Variable | Meaning |
|---|---|
| `AUTH_JWKS_URL` | The provider's JWKS (`https://`; plain `http://` only for localhost). Turns authentication on. |
| `AUTH_JWKS` | The JWKS JSON itself, instead of a URL. |
| `AUTH_ISSUER`, `AUTH_AUDIENCE` | Required with either: the `iss` and `aud` a token must carry. |
| `AUTH_REQUIRED_SCOPE` | Optional scope (`scope` string or `scp`) the token must grant. Missing is a 403. |
| `AUTH_ALGORITHMS` | `RS256,ES256` (default). Nothing else exists; `none` and `HS*` cannot be enabled. |
| `AUTH_LEEWAY_SECS` | Clock skew allowed on `exp` and `nbf` (60). |
| `AUTH_JWKS_MIN_REFRESH_SECS`, `AUTH_JWKS_TTL_SECS` | Shortest gap between key fetches (300) and key lifetime (3600). |

Clients then send `Authorization: Bearer <JWT>`. The service starts only with a complete configuration: an issuer or
audience without keys is an error, so a typo cannot leave it open by accident.

## `healthcheck`

```
ccda2fhir healthcheck --addr 127.0.0.1:8080
```

Exit code 0 if `/healthz` answers, 1 otherwise.
