#!/usr/bin/env python3
"""Print a validator OperationOutcome in the text format of the validator CLI.

Usage: oo_to_text.py <outcome.json>
Lines look like "Error @ Bundle.entry[2].resource (line 240, col8): message", followed by one
summary line, "Success: ..." or "*FAILURE*: ...". Exit status 1 when there are errors.
"""
import json
import sys

LINE = "http://hl7.org/fhir/StructureDefinition/operationoutcome-issue-line"
COL = "http://hl7.org/fhir/StructureDefinition/operationoutcome-issue-col"


def ext(issue, url):
    for e in issue.get("extension", []):
        if e.get("url") == url:
            return e.get("valueInteger")
    return None


def main(path):
    try:
        with open(path) as f:
            outcome = json.load(f)
    except (OSError, ValueError) as e:
        print(f"*FAILURE*: could not read the validator response ({e})")
        return 1
    counts = {"error": 0, "warning": 0, "information": 0}
    lines = []
    for issue in outcome.get("issue", []):
        sev = issue.get("severity", "information")
        if sev == "fatal":
            sev = "error"
        counts[sev] = counts.get(sev, 0) + 1
        where = (issue.get("expression") or issue.get("location") or ["?"])[0]
        line, col = ext(issue, LINE), ext(issue, COL)
        pos = f" (line {line}, col{col})" if line is not None else ""
        text = issue.get("details", {}).get("text") or issue.get("diagnostics") or ""
        label = {"error": "Error", "warning": "Warning"}.get(sev, "Information")
        lines.append(f"  {label} @ {where}{pos}: {text}")
    print("\n".join(lines))
    head = "Success" if counts["error"] == 0 else "*FAILURE*"
    print(f"{head}: {counts['error']} errors, {counts['warning']} warnings, {counts['information']} notes")
    return 0 if counts["error"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
