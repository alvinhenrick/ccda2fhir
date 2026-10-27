# Getting started

ccda2fhir converts a C-CDA document (the XML behind a CCD, discharge summary or referral note) into a FHIR R4
`Bundle`. It is a Rust library, a command line tool, an HTTP service with a small web page, and a WebAssembly
module. They all run the same converter.

## Try it without installing anything

Open the [live demo](../../demo/), drop a C-CDA file on it and read the result. The conversion happens in your
browser and the document is not uploaded. Use synthetic or de-identified documents only.

## Install

You need Rust 1.87 or newer (`rustup` from <https://rustup.rs>).

```
git clone https://github.com/alvinhenrick/ccda2fhir
cd ccda2fhir
cargo build --release -p ccda2fhir-server
```

The binary is `target/release/ccda2fhir`. (Once the crates are published, `cargo install ccda2fhir-server` does
the same.)

## Convert a file

```
ccda2fhir convert ccd.xml > bundle.json
```

The bundle is on standard output. The conversion report (what was dropped, defaulted or approximated) is not part
of it; to see it, run the service with `?report=true` (see [the command line and service](cli-and-service.md))
or use the library (see [using the library](library.md)).

## Run the service

```
ccda2fhir serve --bind 127.0.0.1:8080
```

Open <http://127.0.0.1:8080/> for the web page, or call the API:

```
curl -X POST 'http://127.0.0.1:8080/convert?report=true' \
  -H 'Content-Type: application/xml' --data-binary @ccd.xml
```

Or with Docker: `docker compose up --build` (listens on port 8080).

## What you get back

A FHIR R4 (4.0.1) `Bundle` of type `collection`. The first entries are the `Patient` and a `Composition` that lists
the rest; then one resource per clinical statement (conditions, allergies, medications, observations, procedures,
encounters and so on), plus the practitioners, organizations, locations and `Provenance` they refer to.

Resources declare US Core 6.1.0 profiles in `meta.profile` only where the data needed for the profile is present.
Nothing is invented: if a required element is missing in the source it carries a data-absent-reason, and anything
that was dropped, defaulted or approximated is listed in the report. Read [concepts](concepts.md) next.
