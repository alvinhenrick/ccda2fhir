# Lesson 4: Traits, shared data, and turning a library into a service

Code: `crates/ccda-terminology/src/lib.rs`, `crates/ccda2fhir/src/lib.rs`,
`crates/ccda2fhir-server/src/{server,auth}.rs`. Run `cargo test -p ccda2fhir-server` and read the tests in
`auth.rs` first; they show every behaviour in a few lines each.

The library is pure: bytes in, a result out. This lesson is about what changes when you wrap it in something that
runs for days, serves many callers at once and talks to the network, and which Rust tools keep that safe.

## Rust concepts, in the order you meet them

### 1. A trait is a promise about behaviour
```rust
pub trait Terminology {
    fn system_for_oid(&self, oid: &str) -> String;
    fn official_display(&self, system: &str, code: &str) -> Option<String>;
    // ...
}
```
The mappers never ask "which terminology?"; they ask for these methods. `BuiltinTerminology` answers from tables
in the binary; `LoincTable` answers the same questions from a LOINC file the user supplies. Adding a third
source needs no change in the mappers.

### 2. `&dyn Trait`: choosing the implementation at run time
```rust
let terminology: &dyn Terminology = match options.loinc.as_deref() {
    Some(table) => table,
    None => &BuiltinTerminology,
};
```
Both arms must have one type, so we name it: "a reference to anything that implements `Terminology`". The cost is
one indirect call per method, which is nothing next to parsing XML. The alternative, a generic parameter
`<T: Terminology>`, would compile a copy of every mapper per implementation. For a choice made once per request,
`dyn` is the simpler tool.

### 3. `Arc`: sharing without copying
The service keeps one LOINC table (tens of megabytes) and every request needs to read it. `Option<Arc<LoincTable>>`
in `Config` means "shared, read-only, freed when the last user is done". Cloning an `Arc` copies a pointer and
bumps a counter; it never copies the table. Use `Arc` when several owners must outlive each other in an order you
cannot predict, which is exactly what concurrent requests are.

### 4. `Mutex`: sharing something that changes
The key cache in `auth.rs` is rewritten when keys rotate, so it sits behind a `Mutex`. Two habits matter:
```rust
fn lock(&self) -> std::sync::MutexGuard<'_, Cache> {
    self.cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
```
A lock is "poisoned" if a thread panicked while holding it. Here the cache is a plain value that is never left
half-updated, so we take the data anyway instead of panicking in every later request (the project forbids
`unwrap` outside tests). The second habit: **never hold a `std::sync::Mutex` guard across an `.await`**. In
`refresh` the lock is taken in a small block, the timestamp is written, the guard is dropped at the closing brace,
and only then does the slow network fetch start.

### 5. `async` and the one rule to remember
An `async fn` does not run until something awaits it, and while it waits the thread serves other requests. That
only works if nothing in it *blocks* the thread. The JWKS fetch uses a blocking HTTP client, so it is wrapped:
```rust
tokio::task::spawn_blocking(move || fetch(&url)).await
```
This moves the blocking call to a thread pool made for it. The pure conversion has the same shape (CPU-heavy work
inside a request), which is why the handler also runs it off the async threads.

### 6. Extractors and middleware in axum
A handler declares what it needs in its argument list (`State`, `Query`, `Bytes`, `HeaderMap`) and axum builds
those from the request. Middleware is a function that sees the request first:
```rust
pub async fn require(State(auth): State<Arc<Authenticator>>, request: Request, next: Next) -> Response {
    // ... check the token ...
    next.run(request).await      // let it through
}
```
`route_layer` attaches it to one route, so `/healthz` and the web page stay open while `/convert` is guarded.
Order matters: the layers added last wrap the others, so the body-size limit and timeout still apply before the
handler runs.

### 7. Parsing untrusted input with `?` on `Option`
```rust
let mut parts = token.split('.');
let (header, payload, sig) = (parts.next()?, parts.next()?, parts.next()?);
```
Inside a function that returns `Option`, `?` returns `None` as soon as one piece is missing. `parse_token` is a
chain of these: any malformed step gives `None`, and the caller turns that into a single answer, 401. There is no
way to forget a check, because there is no value to use until every step has succeeded.

### 8. Make wrong states impossible
`Algorithm` has two variants, `Rs256` and `Es256`. A token that says `"alg":"none"` or `"HS256"` cannot even be
represented, so no later code can accidentally accept it. When a security rule can be expressed as "this value does
not exist in the type", prefer that over an `if` somebody could delete.

### 9. Testing a server without a network
```rust
let response = router(&config).oneshot(request).await.unwrap();
```
`router` returns a `tower::Service`, so a test can send a request straight into it and read the response: no port,
no sockets. The one test that needs a socket (`keys_are_fetched_from_a_url_...`) starts a tiny `TcpListener`
thread on `127.0.0.1:0` (the OS picks a free port) and counts how many times it was asked. For signed tokens the
tests ask the `ring` crate to generate a fresh ES256 key pair, so no private key is ever committed.

## What to notice about the design
- **Fail at start, not at 3 a.m.** `AuthArgs::build` refuses to start if an issuer is set without keys. A typo
  must not silently leave the service open.
- **Bound everything that talks outward**: 5 s timeout, 1 MiB size cap, no redirects, at most one refetch per
  interval (failures included) so junk tokens cannot make us hammer the identity provider.
- **Say little when saying no.** A bad token gets 401 and a missing scope 403; neither echoes the token or says
  *why* the token failed. A test checks that a secret in the header never appears in the response.
- **Dependencies are a cost.** We used `ring` for verification instead of a JWT crate because the latter pulled in
  two security advisories with no fix. Checking `cargo deny` before adding a crate saved a worse problem later.

## Exercises
1. Add a method `fn name(&self) -> &'static str` to `Terminology` and implement it for both types. What did the
   compiler make you touch? Now make it a default method; what changes?
2. In `auth.rs`, `Authenticator::check` clones the candidate keys out of the lock (`.cloned()`). Why is that
   better than returning references? What would the compiler say if you tried?
3. Write a test that a token with `nbf` in the future is refused (hint: `verify_with` takes `now` as a
   parameter precisely so tests do not wait).
4. Add `PS256` support. List every place that must change, then check them against the compiler's complaints.
   Which part of the design made the list short?
5. Break it: move the `fetch` call out of `spawn_blocking`, then send many requests with unknown key ids to a slow
   key server. What happens to `/healthz`?

## Next lesson
Turning the library into a WebAssembly module, and what has to change when there is no operating system underneath.
