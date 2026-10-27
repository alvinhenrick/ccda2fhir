# ccda2fhir-server

The `ccda2fhir` command line tool and HTTP service.

```
cargo install ccda2fhir-server

ccda2fhir convert ccd.xml > bundle.json
ccda2fhir serve --bind 127.0.0.1:8080      # web page at /, API at POST /convert
```

`POST /convert` takes the C-CDA as XML, base64 JSON or a FHIR `DocumentReference` and returns a FHIR R4 `Bundle`; `?report=true` adds the conversion status and report. Optional JWT bearer authentication (RS256 and ES256, keys from a JWKS), size and time limits, health endpoints and a built-in web page.

Run it behind TLS. See the [user guide](https://alvinhenrick.github.io/ccda2fhir/book/docs/guide/cli-and-service.html) for every option.

Part of [ccda2fhir](https://github.com/alvinhenrick/ccda2fhir), an open source C-CDA to FHIR R4 converter. MIT licensed. Use synthetic or de-identified data until you have done your own security and compliance review.
