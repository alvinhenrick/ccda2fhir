# Benchmarks

A quick, honest comparison with Medplum's C-CDA converter (`@medplum/ccda` 5.2.9, Node 22), run on the 19
complete documents of [HL7/C-CDA-Examples](https://github.com/HL7/C-CDA-Examples) (1.45 MB in total).
Machine: 2 vCPU Intel Xeon @ 2.8 GHz, one thread each, warm (a first pass is discarded).

## Coverage

| | Documents converted |
|---|---|
| ccda2fhir 0.1.0 | 19 of 19 |
| Medplum 5.2.9 | 8 of 19 |

Medplum throws on the other 11: ten with "Unhandled act templateId" or "Unhandled substance administration
templateId" (planned activities, hospital-course wrappers and similar sections) and one with "section is not
iterable". ccda2fhir maps those templates or reports them as unmapped, and never stops the conversion.

## Speed, on the 8 documents both can convert

| | Per document |
|---|---|
| ccda2fhir (in process, `bench` example) | about 2.1 ms |
| Medplum (in process, XML to FHIR) | about 6.7 ms |

About 3 times faster. Over all 19 documents ccda2fhir runs at roughly 450 documents per second (33 MB/s) on one core.

## Reproduce

```
cargo run --release -p ccda2fhir --example bench -- --rounds 20 path/to/documents/*.xml
```

For Medplum: `npm i @medplum/ccda`, then time `convertCcdaToFhir(convertXmlToCcda(xml))` over the same files.

## Caveats

- One machine, one run of each, small corpus. Treat the ratio as "a few times", not a precise figure.
- Entry counts are not a quality measure. We emit more resources per document (Composition, Provenance, the
  practitioners and organizations the data refers to); quality is judged by the HL7 validator in CI.
