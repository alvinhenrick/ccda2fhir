#!/usr/bin/env bash
# Validate converter output with the official HL7 FHIR validator.
#
# STATUS: written but not yet run by the author's tooling (the sandbox it was written in blocked
# downloading and executing the validator jar). Run it yourself once and report what it says.
#
# Usage: scripts/validate.sh [fixture-name ...]    (default: every fixture)
# Needs: Java 17+, curl, network access to github.com, packages.fhir.org and tx.fhir.org.
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

status=0
for name in "${names[@]}"; do
  # Emit each bundle entry as its own file so the validator reports per resource.
  cargo run -q -p ccda2fhir --example convert_file -- \
    "$ROOT/crates/ccda2fhir/tests/fixtures/$name.xml" "$OUT/$name.bundle.json"
  java -jar "$JAR" "$OUT/$name.bundle.json" -version "$FHIR_VERSION" -ig "$US_CORE_PACKAGE" \
    | tee "$OUT/$name.validation.txt" || status=1
done
exit $status
