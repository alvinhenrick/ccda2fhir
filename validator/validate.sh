#!/usr/bin/env bash
# Convert every *.xml in /input with the converter service, then validate each bundle.
# Exits non-zero if any bundle has validation errors or any document fails to convert.
set -uo pipefail

SERVICE="${CONVERTER_URL:-http://ccda2fhir:8080}"
FHIR_VERSION="${FHIR_VERSION:-4.0.1}"
US_CORE_PACKAGE="${US_CORE_PACKAGE:-hl7.fhir.us.core#6.1.0}"
OUT=/work
mkdir -p "$OUT"

status=0
count=0
for doc in /input/*.xml; do
  [ -e "$doc" ] || { echo "no .xml files in /input"; exit 2; }
  name="$(basename "$doc" .xml)"
  count=$((count + 1))
  if ! curl -fsS -X POST -H 'Content-Type: application/xml' --data-binary "@$doc" \
        "$SERVICE/convert" -o "$OUT/$name.bundle.json"; then
    echo "FAILED to convert $name"
    status=1
    continue
  fi
  echo "== validating $name"
  java -jar /opt/validator_cli.jar "$OUT/$name.bundle.json" \
    -version "$FHIR_VERSION" -ig "$US_CORE_PACKAGE" | tee "$OUT/$name.validation.txt"
  if [ "${PIPESTATUS[0]}" -ne 0 ]; then status=1; fi
done
echo "validated $count document(s); status $status"
exit $status
