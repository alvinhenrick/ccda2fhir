# Using the library

Add the facade crate (not yet on crates.io: use a path or git dependency for now):

```toml
[dependencies]
ccda2fhir = { git = "https://github.com/alvinhenrick/ccda2fhir" }
```

## Convert a document

```rust
use ccda2fhir::{convert, Options};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let xml = std::fs::read("ccd.xml")?;
    let conversion = convert(&xml, &Options::default())?;

    // The FHIR bundle as serde_json::Value.
    let bundle = conversion.bundle.to_json();
    println!("{}", serde_json::to_string_pretty(&bundle)?);

    // What was lossy or unexpected.
    for issue in &conversion.report.issues {
        eprintln!("{:?} {}: {}", issue.severity, issue.code, issue.message);
    }
    Ok(())
}
```

`convert` takes bytes and returns a `Conversion` or a `ConvertError`. It performs no I/O, keeps no state and is safe to
call from many threads at once.

## The result

| Item | Meaning |
|---|---|
| `conversion.bundle` | The FHIR `Bundle`. `to_json()` gives a `serde_json::Value`. |
| `conversion.report` | Every judgment call, as `Issue { severity, code, message }`. |
| `conversion.status()` | `"complete"` (no warnings) or `"partial"` (at least one warning). |
| `conversion.warnings()` | How many report entries are warnings. |
| `conversion.to_response_json()` | `{ status, issues, bundle, report }`, the shape the service returns with `?report=true`. |

`Severity::Info` means no data was lost; `Severity::Warning` means data was dropped, defaulted or approximated.
Report codes are stable (`condition.status.defaulted`, `entry.unmapped`, ...). Messages never contain document
values, so they are safe to log.

## Options

```rust
use ccda2fhir::{Limits, Options, Profiles};

let options = Options {
    profiles: Profiles::None,                 // leave out meta.profile (default: UsCore610)
    limits: Limits { max_bytes: 5 * 1024 * 1024, ..Limits::default() },
    ..Options::default()
};
```

| Option | Default | Meaning |
|---|---|---|
| `profiles` | `Profiles::UsCore610` | Profiles to declare in `meta.profile`. `Profiles::None` declares none. |
| `limits` | 50 MiB, 5,000,000 nodes, depth 256 | Input safety limits applied while parsing. Exceeding one is a `ConvertError`, not a crash. |
| `loinc` | `None` | A `LoincTable` (from your own LOINC download) that sets each result's category. Without it results are laboratory. |

## Errors

`ConvertError::Parse` means the input is too large, not UTF-8, not well-formed XML, has a `DOCTYPE`, or is not a
C-CDA `ClinicalDocument`. Its message never includes document content. Data problems inside a valid document are not
errors; they are report entries.

## Determinism

The same document and options always give the same bundle: ids are sequential per resource type
(`condition-1`, `condition-2`, ...) and every `fullUrl` is a UUID v5 derived from the document id, so bundles from
different documents never collide and re-running a conversion gives identical output.
