#!/usr/bin/env bash
# Assemble the documentation site in target/site:
#
#   /            landing page (site/index.html)
#   /book/       mdBook built from README, docs/, LESSONS/ and the root documents
#   /api/        rustdoc for the library crates
#   /demo/       the web page running the WebAssembly build in the browser
#
# Needs: mdbook, and (for the demo) the web build from `scripts/build-wasm.sh web`.
# Set SKIP_DEMO=1 to leave the demo out.
set -euo pipefail
cd "$(dirname "$0")/.."

out=target/site
rm -rf "$out" target/book-src
mkdir -p "$out"

cp site/index.html "$out/index.html"
touch "$out/.nojekyll"

# API reference. The service crate is left out: its binary is also called `ccda2fhir`, which
# collides with the library's page.
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --exclude ccda2fhir-server
cp -r target/doc "$out/api"

# Book: the repository's own documents, kept where they are so relative links keep working.
src=target/book-src
mkdir -p "$src"
cp README.md CHANGELOG.md DESIGN.md SECURITY.md CONTRIBUTING.md "$src/"
cp -r docs LESSONS "$src/"
{
  echo "# Summary"
  echo
  echo "[Overview](README.md)"
  echo "[Changelog](CHANGELOG.md)"
  echo "[Design](DESIGN.md)"
  echo "[Security](SECURITY.md)"
  echo "[Contributing](CONTRIBUTING.md)"
  echo
  echo "# Reference"
  echo
  for f in docs/uscdi.md docs/coverage.md docs/benchmarks.md; do
    [ -f "$f" ] || continue
    echo "- [$(head -n 1 "$f" | sed 's/^# *//')]($f)"
  done
  echo
  echo "# Lessons"
  echo
  for f in LESSONS/*.md; do
    echo "- [$(head -n 1 "$f" | sed 's/^# *//')]($f)"
  done
  echo
  echo "# Design decisions"
  echo
  for f in docs/decisions/*.md; do
    echo "- [$(head -n 1 "$f" | sed 's/^# *//')]($f)"
  done
} > "$src/SUMMARY.md"
cat > "$src/book.toml" <<'T'
[book]
title = "ccda2fhir"
language = "en"
src = "."

[output.html]
git-repository-url = "https://github.com/alvinhenrick/ccda2fhir"
T
mdbook build "$src" -d "$PWD/$out/book"

# Demo: the same page the service serves, with the in-browser engine.
if [ "${SKIP_DEMO:-0}" != "1" ]; then
  ui=crates/ccda2fhir-server/src/ui
  wasm=target/wasm-web
  [ -f "$wasm/ccda2fhir_wasm.js" ] || { echo "run scripts/build-wasm.sh web first (or set SKIP_DEMO=1)" >&2; exit 1; }
  mkdir -p "$out/demo/ui/wasm"
  cp "$ui/app.js" "$ui/app.css" "$out/demo/ui/"
  cp "$wasm"/ccda2fhir_wasm.js "$wasm"/ccda2fhir_wasm_bg.wasm "$out/demo/ui/wasm/"
  # Pages cannot set headers, so the policy goes in a meta tag. 'wasm-unsafe-eval' is what lets the
  # browser compile the module; scripts still come only from this site.
  csp="default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'"
  sed "s|<meta charset=\"utf-8\">|<meta charset=\"utf-8\">\n<meta http-equiv=\"Content-Security-Policy\" content=\"$csp\">\n<meta name=\"engine\" content=\"wasm\">|" \
    "$ui/index.html" > "$out/demo/index.html"
fi

echo "site built in $out"
