# 69. A WebAssembly build

Date: 2026-10-26. Status: accepted.

## Decision

- `crates/ccda2fhir-wasm` wraps the facade with `wasm-bindgen`: `convert_ccda(xml, profiles)` returns
  the same JSON as `POST /convert?report=true` (`{ status, issues, bundle, report }`) and throws a
  content-free error if the document cannot be parsed. `version()` reports the converter version.
  The library, mappers and terminology tables are exactly the ones the service uses, so there is
  one implementation, not a port.
- The response shape moved into the facade (`Conversion::status`, `warnings`, `to_response_json`) so
  the service and the module cannot drift apart.
- `scripts/build-wasm.sh [web|nodejs]` builds and binds it. It checks that the installed
  `wasm-bindgen` CLI matches the version in `Cargo.lock`, because a mismatch fails at run time with
  an unhelpful error.
- CI builds both targets, then runs `scripts/wasm-smoke.cjs` in Node: every fixture that has a
  golden file must convert to a bundle **identical** to the native golden. That proves the
  WebAssembly build behaves like the native one (same ids, same order, same decimals).
- `ccda2fhir-wasm` is not published to crates.io (`publish = false`); it is a build artefact for
  npm or a static site.

## Notes

- The workspace forbids `unsafe`; `wasm-bindgen` generates some at the JavaScript boundary, and `forbid`
  cannot be lifted for generated code, so this one crate keeps the other workspace lints and drops that
  one. Nothing we write in it is unsafe.
- No `getrandom` or clock is needed: ids are UUID v5 from the document, which is also why output is
  identical between native and WebAssembly.
- No limits changed: the parser's size and node limits apply, but the browser tab's memory is the real
  ceiling for large documents.

## Not done

An npm package, and wiring the built-in web page to convert locally instead of calling the server.
