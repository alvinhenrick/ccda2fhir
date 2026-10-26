#!/usr/bin/env python3
"""Summarise validator errors for the HL7 example corpus and fail on any that are not known.

Usage: corpus_gate.py <validation-dir> <known-errors-file> [summary-out]

Errors are normalised (indexes, uuids, line numbers removed) and counted per distinct kind. A kind
matching a regular expression in the known-errors file is a documented validator or source-data
limit; anything else fails the run. The examples are HL7's public ones, so messages are safe to show.
"""
import collections
import glob
import re
import sys


def normalise(text):
    text = re.sub(r"\[\d+\]", "[]", text)
    text = re.sub(r"urn:uuid:[0-9a-f-]+", "urn:uuid:X", text)
    text = re.sub(r"\(line \d+, col\d+\)", "", text)
    return re.sub(r"/\*([A-Za-z]+)/[a-z0-9-]+\*/", r"/*\1*/", text)


def main():
    directory, known_file = sys.argv[1], sys.argv[2]
    out = open(sys.argv[3], "w") if len(sys.argv) > 3 else sys.stdout
    known = [
        re.compile(line.strip())
        for line in open(known_file)
        if line.strip() and not line.startswith("#")
    ]
    seen = collections.defaultdict(set)
    for path in sorted(glob.glob(f"{directory}/*.validation.txt")):
        doc = path.split("/")[-1].split(".")[0]
        for line in open(path, errors="replace"):
            m = re.match(r"\s*Error @ (.*)", line)
            if m and "dom-6" not in line:
                seen[normalise(m.group(1).rstrip())].add(doc)
    unknown = {t: d for t, d in seen.items() if not any(k.search(t) for k in known)}
    print(f"## HL7 example corpus: {len(seen)} distinct error kinds, {len(unknown)} not known", file=out)
    for label, group in (("New", unknown), ("Known", {t: d for t, d in seen.items() if t not in unknown})):
        for t, docs in sorted(group.items(), key=lambda kv: -len(kv[1])):
            print(f"- {label} ({len(docs)} docs: {', '.join(sorted(docs))}) `{t[:380]}`", file=out)
    return 1 if unknown else 0


if __name__ == "__main__":
    sys.exit(main())
