#!/usr/bin/env bash
# Build the WebAssembly module and its JavaScript bindings.
#
#   scripts/build-wasm.sh [web|nodejs] [out-dir]
#
# Needs: rustup target add wasm32-unknown-unknown
#        cargo install wasm-bindgen-cli --locked --version <the wasm-bindgen version in Cargo.lock>
# `web` (default) produces an ES module for browsers; `nodejs` a CommonJS module.
set -euo pipefail
cd "$(dirname "$0")/.."

target="${1:-web}"
out="${2:-target/wasm-$target}"

want="$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/[version= "]/,""); print; exit}' Cargo.lock)"
have="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)"
if [ "$have" != "$want" ]; then
  echo "wasm-bindgen CLI is '${have:-missing}' but Cargo.lock needs $want. Run:" >&2
  echo "  cargo install wasm-bindgen-cli --locked --version $want" >&2
  exit 1
fi

cargo build -p ccda2fhir-wasm --target wasm32-unknown-unknown --release --locked
wasm-bindgen --target "$target" --out-dir "$out" \
  target/wasm32-unknown-unknown/release/ccda2fhir_wasm.wasm
ls -l "$out"
