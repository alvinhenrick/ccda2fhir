# 0027: Display names

Status: accepted (first step). Settles the open "displayName policy" question.

Real documents often carry a wrong, shortened or old display for a well-known code, and the HL7 validator rejects a wrong display.
The goal is to keep every piece of data and still produce valid FHIR.

- If the code is in the library's official-display table (`ccda_terminology::official_display`) and the document's display
  differs, `coding.display` is the official one. The document's own wording is kept as the concept's `text` (unless the document
  already gave an `originalText`) and the report gets `coding.display.replaced` (info, no document wording in it).
- If the code is not in the table, the document's display is kept unchanged. We never guess a display.
- The table only holds rows checked against the HL7 validator; it starts with the LOINC section codes. It grows from validator
  findings and the data-loss audit. Licensed or large systems (SNOMED CT, RxNorm, CPT) are not bundled: SNOMED and RxNorm can be
  supplied at run time through the `Terminology` trait; CPT displays are never bundled (AMA license).
