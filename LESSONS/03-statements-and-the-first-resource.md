# Lesson 3: A generic tree, narrative references, and our first resource (Problems to Condition)

Code: `crates/ccda-model/src/{body,types,xml,header}.rs`, `crates/ccda-map/src/{context,quantity,problem}.rs`,
`crates/ccda2fhir/tests/golden.rs`. Run `cargo test --workspace`; read the `problem.rs` tests first.

## Rust concepts, in the order you meet them

### 1. Recursive types need no box here
```rust
pub struct Statement { /* ... */ pub related: Vec<Related> }
pub struct Related { pub link: Link, pub statement: Statement }
```
A struct cannot contain itself directly (its size would be infinite), but a `Vec` stores its items on the
heap, so `Statement -> Vec<Related> -> Statement` is fine. If you needed a single child you would write
`Option<Box<Statement>>`. Recursion in the *parser* is bounded with a `depth` argument, because untrusted input
decides how deep the tree goes.

### 2. Enums that carry data
`Value` is `Coded(Code) | Quantity(Quantity) | Integer(Scalar) | ... | Other { xsi_type }`. The parser picks the
variant from `xsi:type`; mappers `match` on it and the compiler forces them to say what happens to the others.
`Other` exists so an unmodelled type is *reported* rather than silently lost.

### 3. `let ... else` with a pattern
```rust
let Some(Cv::Coded(value)) = status.values.first() else { return None; };
```
`first()` gives `Option<&Value>`; the pattern looks through both layers at once.

### 4. Lifetimes in return types
`related_by<'a>(&'a self, type_code: &'a str) -> impl Iterator<Item = &'a Statement>` says: the iterator borrows
from *both* `self` and `type_code`. In edition 2021 the compiler cannot guess that, so we spell it out (the first
compile error of this phase).

### 5. `BTreeMap` for determinism
The narrative lookup and the id counters use `BTreeMap`, which iterates in key order. `HashMap` order changes from
run to run, which would make output non-deterministic. Same reason `serde_json` prints object keys sorted.

### 6. A context struct instead of many parameters
`Context` bundles terminology, the report and the id source. Mappers take `&mut Context`, so adding a new shared
service later does not change every function signature. `next_id(&mut self, ...)` shows interior counting with
`entry(...).or_insert(0)`.

### 7. Exact decimals
`"5.40"` parsed through `f64` becomes `5.4` and loses significant digits, which FHIR treats as meaningful. We
validate the text against the decimal grammar, then let `serde_json` (with `arbitrary_precision`) keep the digits
exactly as written. A test pins it.

### 8. Test through the real parser
The `problem.rs` tests build small XML strings and run `ccda_model::parse` first, so they exercise model and mapper
together. Helper functions (`document`, `observation`, `status_obs`) keep each test to the one thing it is about.

### 9. Golden files
`tests/golden/problems.json` is the reviewed expected output for `tests/fixtures/problems.xml`. A mismatch fails the
test; `UPDATE_GOLDEN=1` rewrites it, and you read the diff like any code change.

## What to notice about the design
- Every judgment call is a report entry with a stable code, never a silent change: `condition.status.defaulted`,
  `condition.status.adjusted-for-abatement`, `condition.negated`, `condition.onset.age-ignored`.
- Rules trace to IG rows; `docs/coverage.md` lists each row with its test.
- The mapper selects by `templateId` and reads the category from the section's LOINC code, exactly as the IG says.

## Exercises
1. A Problem Observation with `negationInd="true"` and value `55607006` ("Problem"): what does the IG want? Read
   `CF-problems.md`, write the test first, then decide.
2. Add `Comment Activity` (`48767-8`) to `Condition.note`. Find the IG's "Comment to Annotation" section first.
3. Make `walk` in `ClinicalDocument::all_sections` iterative and prove with a test it gives the same order.
4. Break it: remove the `depth > MAX_DEPTH` check and feed the `rejects_runaway_nesting` input. What happens?

## Next lesson
Authors and Provenance (who recorded this, and when), then Allergies.
