#!/usr/bin/env bash
# Validate converter output with the official HL7 FHIR validator.
#
# The validator runs once as a web service (`-server`) and every bundle is posted to it, which
# avoids a Java start and an IG load per fixture.
#
# Usage: scripts/validate.sh [fixture-name ...]    (default: every fixture)
# Needs: Java 17+, curl, python3, network access to github.com, packages.fhir.org and tx.fhir.org.
set -euo pipefail

VALIDATOR_VERSION="${VALIDATOR_VERSION:-7.0.3}"
FHIR_VERSION="4.0.1"
US_CORE_PACKAGE="${US_CORE_PACKAGE:-hl7.fhir.us.core#6.1.0}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CACHE="$ROOT/.cache/validator"
JAR="$CACHE/validator_cli-$VALIDATOR_VERSION.jar"
OUT="$ROOT/target/validation"

mkdir -p "$CACHE" "$OUT"
if [ ! -f "$JAR" ]; then
  curl -fsSL -o "$JAR" \
    "https://github.com/hapifhir/org.hl7.fhir.core/releases/download/$VALIDATOR_VERSION/validator_cli.jar"
  echo "downloaded validator $VALIDATOR_VERSION, sha256: $(sha256sum "$JAR" | cut -d' ' -f1)"
fi

names=("$@")
if [ ${#names[@]} -eq 0 ]; then
  for f in "$ROOT"/crates/ccda2fhir/tests/fixtures/*.xml; do names+=("$(basename "$f" .xml)"); done
fi

# Build the converter once, then convert every fixture with the binary.
cargo build -q -p ccda2fhir --example convert_file
CONVERT="$ROOT/target/debug/examples/convert_file"
for name in "${names[@]}"; do
  "$CONVERT" "$ROOT/crates/ccda2fhir/tests/fixtures/$name.xml" "$OUT/$name.bundle.json"
done

# Start the validator once as a web service so FHIR R4 and US Core load a single time, then post
# every bundle to it. PARALLEL requests run at the same time.
PORT="${VALIDATOR_PORT:-8090}"
PARALLEL="${PARALLEL:-4}"
java -jar "$JAR" -server "$PORT" -version "$FHIR_VERSION" -ig "$US_CORE_PACKAGE" \
  >"$OUT/server.log" 2>&1 &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT

echo "waiting for the validator service on port $PORT"
for _ in $(seq 1 180); do
  if curl -s -o /dev/null -m 3 "http://localhost:$PORT/"; then break; fi
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then echo "validator exited"; tail -30 "$OUT/server.log"; exit 1; fi
  sleep 2
done

validate_one() {
  name="$1"
  curl -sS -m 600 -X POST -H 'Content-Type: application/fhir+json' \
    --data-binary "@$OUT/$name.bundle.json" "http://localhost:$PORT/validateResource" \
    -o "$OUT/$name.outcome.json" || echo '{}' >"$OUT/$name.outcome.json"
  python3 "$ROOT/scripts/oo_to_text.py" "$OUT/$name.outcome.json" >"$OUT/$name.validation.txt"
}
export -f validate_one
export OUT PORT ROOT

printf '%s\n' "${names[@]}" | xargs -P "$PARALLEL" -I{} bash -c 'validate_one {}'

status=0
for name in "${names[@]}"; do
  echo "== $name"
  cat "$OUT/$name.validation.txt"
  grep -q '^\*FAILURE\*' "$OUT/$name.validation.txt" && status=1
done
exit $status
