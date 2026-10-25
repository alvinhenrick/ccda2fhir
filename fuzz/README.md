# Fuzzing

Coverage-guided fuzzing of the whole conversion with [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)
(libFuzzer). It needs a nightly toolchain.

```sh
cargo install cargo-fuzz --locked
mkdir -p fuzz/corpus/convert && cp crates/ccda2fhir/tests/fixtures/*.xml fuzz/corpus/convert/
cargo +nightly fuzz run convert -- -max_len=65536 -max_total_time=300
```

A crash is saved under `fuzz/artifacts/convert/`. Reproduce it with
`cargo +nightly fuzz run convert fuzz/artifacts/convert/<file>`, fix the cause, and add the input's
shape (never real patient data) to `crates/ccda2fhir/tests/robustness.rs`.

CI runs a short smoke run on pull requests and a longer run weekly (`.github/workflows/fuzz.yml`).
