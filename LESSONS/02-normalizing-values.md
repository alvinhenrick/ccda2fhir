# Lesson 2: Normalizing timestamps, nullFlavor, identifiers and codes

Code: `crates/ccda-map/src/{timestamp,nullflavor,identifier,coding}.rs`, plus `Code` and identifiers in `ccda-model`.
Run `cargo test --workspace` and read the tests first: they are the specification.

Rules come from the IG, not from memory: see `docs/decisions/0002-timestamp-and-null-handling.md`.

## Rust concepts, in the order you meet them

### 1. Modules and what you export
`ccda-map/src/lib.rs` declares `mod timestamp;` (private) and re-exports chosen items with `pub use`. Callers see
`ccda_map::map_datetime`, never the file layout. A private module plus selective `pub use` is how Rust libraries keep
their public API small and free to reorganize.

### 2. Traits and `&dyn Trait`
```rust
pub fn map_identifier(id: &Identifier, terminology: &dyn Terminology, report: &mut Report) -> Option<Value>
```
`Terminology` is a **trait**: a set of methods a type can implement. `&dyn Terminology` means "a reference to any
type that implements it", decided at run time. The mappers do not care whether OIDs come from the built-in table or a
terminology server. Trait methods can have default bodies (`system_for_oid` is built on `oid_to_uri`).

### 3. Lifetimes in a struct
```rust
struct Parts<'a> { year: &'a str, month: Option<&'a str>, ... }
```
`Parts` holds slices of the input string instead of copying them. `'a` says "these slices live no longer than the input".
The compiler will refuse any code that keeps a `Parts` after the string is gone. This is Rust's version of "no dangling
pointers" and costs nothing at run time.

### 4. `Option` combinators
`and_then`, `map`, `filter`, `unwrap_or`, `then_some`: they replace most `if let` ladders. In `parse_code`,
`original_text.as_ref().filter(|n| child(n, "reference").is_none()).and_then(text_of)` reads as "take the originalText,
keep it only if it has no reference child, then extract its text". `as_ref()` borrows the `Option`'s contents so the
original is not moved.

### 5. Pattern matching on tuples
```rust
match (root, extension) {
    (None, Some(value)) => ...,
    (Some(root), Some(extension)) => ...,
    (root, None) => ...,
}
```
Matching on a tuple of options lists the cases like a truth table, and the compiler checks you covered every
combination. Add a case and forget one and it will not compile.

### 6. `let ... else` for the sad path
```rust
let (Some(hour), Some((sign, off_h, off_m))) = (p.hour, p.offset) else { return Ok(...) };
```
Destructure on the happy path; `else` handles everything that did not match and must leave the function.

### 7. Errors as values, in two tiers
- `Result<Converted, InvalidTimestamp>`: a pure function that can fail. The caller decides what that means.
- `map_datetime(..., report)`: the wrapper that turns failure into a **report entry** and returns `None`.
Data problems become report entries; only structural failures (cannot parse XML at all) are `Err`.

### 8. Table-driven tests
`rejects_bad_values` loops over an array of bad inputs and asserts on each, with the input in the failure message
(`"{bad}"`). One loop, many cases. The identifier tests mirror the IG's table row for row: the spec and the tests
read side by side.

### 9. Iterator tricks
`std::iter::once(code).chain(code.translations.iter()).filter_map(...)` walks the code and then its translations as one
sequence, keeping only the ones that produce a value.

## What to notice about the design
- Nothing here panics on bad input. Every failure path is a tested `None` or a report entry.
- Messages never contain input values; tests assert that with sentinel strings like `Secret`.
- We found a likely issue in the IG itself (its dateTime example omits seconds, which R4 requires). It is written down
  in the ADR to verify with the HL7 validator.

## Exercises
1. Add `fn hl7_ts_to_fhir_instant` for fields typed `instant` (precision must reach the second, offset required).
   Decide what to do with a date-only value and write the tests first.
2. `Identifier` with both `extension` and `nullFlavor`: what does the code do today? Write a test that pins it, then
   decide if that is right.
3. Pick one more code system or identifier system and add it to the terminology table, but only after you find its
   OID and URI in the HL7 registry or the FHIR terminologies page. Practice the habit: no mapping without a reference.
4. Break something on purpose: change `is_multiple_of(400)` to `is_multiple_of(40)` and see which test catches it.

## Next lesson
Walking sections and entries by `templateId`, and our first real resource: Problems to `Condition`.
