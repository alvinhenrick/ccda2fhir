# ccda2fhir

Convert C-CDA documents to FHIR R4 bundles. A pure library: bytes in, a bundle and a report out. No I/O, no state, deterministic output.

```rust
use ccda2fhir::{convert, Options};

let xml = std::fs::read("ccd.xml")?;
let conversion = convert(&xml, &Options::default())?;
println!("{}", conversion.bundle.to_json());
for issue in &conversion.report.issues {
    eprintln!("{:?} {}: {}", issue.severity, issue.code, issue.message);
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

Nothing is dropped silently: every judgment call is a report entry with a stable code and a message that never contains document values. Resources claim US Core 6.1.0 profiles only where the data they require is present.

This is the crate most programs want. The command line tool and HTTP service are in `ccda2fhir-server`.

Part of [ccda2fhir](https://github.com/alvinhenrick/ccda2fhir), an open source C-CDA to FHIR R4 converter. MIT licensed. Use synthetic or de-identified data until you have done your own security and compliance review.
