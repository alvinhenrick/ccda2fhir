# 68. A built-in web page

Date: 2026-10-26. Status: accepted.

## Decision

- The service serves a small page at `/` (with `/ui/app.js` and `/ui/app.css`): paste or drop a
  C-CDA, convert, and read the result as a resource list (filter by type, click for the JSON), the
  conversion report, and the bundle JSON with copy and download. It calls `POST /convert?report=true`
  on the same origin.
- The files are embedded in the binary (`include_str!`). No framework, no build step, no network
  access beyond the service, so the Docker image and the single binary stay self-contained.
- Strict headers on every asset: `Content-Security-Policy: default-src 'none'; script-src 'self';
  style-src 'self'; connect-src 'self'; ...; frame-ancestors 'none'`, `nosniff`, `no-referrer`. The
  script and style are separate files so no `unsafe-inline` is needed. All dynamic text goes in with
  `textContent`, never `innerHTML`, so document content cannot become markup.
- The page is not behind authentication (it is static and holds nothing). `/convert` is, and the
  page has a field for a bearer token, kept in memory and never stored.
- `DISABLE_UI=1` or `serve --no-ui` removes the routes, for deployments that want an API only.

## Not done

Sign-in flows (OIDC redirect), saving documents or results, comparing two conversions, and
rendering the FHIR narrative. The page is a developer and demo tool, not a clinical viewer; see
SECURITY.md for the data-handling rules.
