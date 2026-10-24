# Lesson 1: Parsing the C-CDA header

Code: `crates/ccda-model/src/lib.rs`. Run it with `cargo test` from the repo root.

## What we built
A function `parse(&[u8]) -> Result<ClinicalDocument, ParseError>` that reads a C-CDA document and returns a typed
struct with the document id, effective time and the patient's name, birth date and gender, with safety limits.

## Rust concepts, in the order the code uses them

### 1. Workspaces and crates
`Cargo.toml` at the root declares a **workspace**: several crates (packages) built together. Shared settings live in
`[workspace.package]` and `[workspace.dependencies]`, and each crate opts in with `version.workspace = true`.
`ccda-model` is a **library crate** (`src/lib.rs`), so other crates can depend on it later.

### 2. Structs and derives
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier { pub root: String, pub extension: Option<String> }
```
A `struct` groups named fields. `#[derive(...)]` asks the compiler to generate common behavior: `Debug` (printing for
tests), `Clone` (explicit copies), `PartialEq`/`Eq` (the `==` operator, which `assert_eq!` needs).
`pub` makes an item visible outside the module.

### 3. `Option<T>`: absence without null
Rust has no null. A value that may be missing is `Option<String>`: either `Some("x")` or `None`. The compiler forces
you to handle both. This fits C-CDA well, where almost everything is optional.

### 4. `Result<T, E>` and the `?` operator
```rust
let text = std::str::from_utf8(xml).map_err(|_| ParseError::NotUtf8)?;
```
`Result<T, E>` is either `Ok(value)` or `Err(error)`. `?` means "if this is an error, return it from the function now;
otherwise unwrap the value." `map_err` converts one error type into ours. No exceptions anywhere.

### 5. Enums for errors, and `thiserror`
`ParseError` is an **enum**: a value that is exactly one of several variants, some carrying data
(`TooLarge { size, limit }`). The `thiserror` derive writes the `Display` message from the `#[error("...")]` text.
Design rule from `DESIGN.md`: messages never include document content, and a test enforces it.

### 6. Borrowing and lifetimes (the big Rust idea)
```rust
fn child<'a, 'input>(node: &Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>>
```
The parser (`roxmltree`) does not copy the XML: nodes *borrow* from the input text. `'a` and `'input` are
**lifetimes**, labels telling the compiler "the returned node is only valid as long as the document it came from."
You rarely write these; here they are needed because we return a node that points into the document. Mental model:
the compiler proves you never use data after it is gone, so there are no dangling pointers and no garbage collector.

`&str` is a borrowed string slice; `String` is an owned, growable string. We use `.to_owned()` to turn borrowed text
into owned `String`s so the result outlives the parsed document.

### 7. Iterators and closures
```rust
node.children().find(|c| is_hl7(c, "id"))
```
`children()` returns an iterator; `find` takes a **closure** (`|c| ...`, an inline function) and returns the first
match as an `Option`. Chains like `.filter_map(...).collect()` replace most loops. `try_fold` in `path()` walks a list
of names, stopping at the first missing one.

### 8. `let ... else` and early return
```rust
let Some(patient) = path(...) else { return Patient::default(); };
```
Destructure or bail out. Great for "missing optional section" cases.

### 9. Tests live next to the code
`#[cfg(test)] mod tests` is compiled only for `cargo test`. Each `#[test]` function is a test; `assert_eq!` compares
values. The tests cover the happy path, missing data, wrong root, an entity-expansion attack, size limits, invalid
UTF-8, and PHI-free errors.

## Why the safety choices matter
- `allow_dtd: false` rejects any DOCTYPE, which blocks "billion laughs" entity bombs.
- `nodes_limit` and `max_bytes` bound memory.
- Namespace checks (`urn:hl7-org:v3`) avoid mistaking look-alike elements for C-CDA ones.

## Exercises
1. Add `Patient.addr` (street, city, state, postal code). Which fields are `Option`? Write a test first.
2. Add `ClinicalDocument.custodian_name` from `custodian/assignedCustodian/representedCustodianOrganization/name`.
   Reuse `path()` and `text_of()`.
3. Make a document with two `<id>` elements. Which does `child()` return? Decide whether that is right for C-CDA
   and write a test that pins the behavior.
4. Try removing `.to_owned()` somewhere and read the compiler's error. The message explains the lifetime problem.

## Next lesson
Normalizing values (HL7 timestamps with partial precision, `nullFlavor`) and the `Result` vs warning split.
