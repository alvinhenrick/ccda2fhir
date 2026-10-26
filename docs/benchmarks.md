# Benchmarks

Throughput of the converter itself: parsing and mapping in one process, no I/O, warm (a first pass is
discarded). Corpus: the 19 complete documents of [HL7/C-CDA-Examples](https://github.com/HL7/C-CDA-Examples)
(1.45 MB in total, 19 of 19 convert).

| | |
|---|---|
| Documents per second | about 450 |
| Throughput | about 33 MB/s |
| Time per document | about 2.3 ms on average |

Machine: 2 vCPU Intel Xeon @ 2.8 GHz, one thread.

## Reproduce

```
cargo run --release -p ccda2fhir --example bench -- --rounds 20 path/to/documents/*.xml
```

## Caveats

One machine, one run, a small corpus: treat the figures as the right order of magnitude, not a precise
measurement. The HTTP service adds request handling and JSON output on top of these numbers.
