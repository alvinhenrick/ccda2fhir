# 0021: Hardening against hostile and messy input

Status: accepted.

- **Depth.** The XML parser recurses on element depth, and a 100,000-deep document overflowed the stack even though our own
  tree walkers were capped. `Limits::max_depth` (default 256; real documents stay under about 100) is checked with a flat scan before
  parsing and returns `ParseError::TooDeep`. The scan follows comments, CDATA, processing instructions and quoted attribute values.
- **Robustness tests** (`crates/ccda2fhir/tests/robustness.rs`) run on every CI build: truncation at 129 points, random byte
  damage (150 seeds per fixture), hostile attribute values (huge numbers, bad dates, odd Unicode, NUL), 200x repeated bodies,
  deep nesting, 8 MiB values, entity expansion and other hostile markup. They check: no panic, no stack overflow, under 10 s,
  the bundle serializes, and no report entry or error contains document text. Every case has a printed id, so failures reproduce.
- **Fuzzing.** `fuzz/` holds a cargo-fuzz (libFuzzer) target for the whole conversion, seeded with the fixtures.
  `.github/workflows/fuzz.yml` runs 60 s on pull requests, 10 minutes weekly, or on demand.
- **Performance.** `cargo run --release -p ccda2fhir --example scale -- <fixture> <n>...` repeats a fixture body and prints time
  per KiB. It found a quadratic duplicate check when a section lists thousands of resources (fixed with a set). Now about
  70 us/KiB (roughly 14 MB/s on the CI-class machine used), flat from 100 KiB to 7 MiB.
- **Still to do:** a corpus of messy real-world (de-identified, redistributable) documents, memory bounds for the 50 MiB limit
  (roughly 5-10x input), and mutation of the mapper input
  (structure-aware fuzzing).
