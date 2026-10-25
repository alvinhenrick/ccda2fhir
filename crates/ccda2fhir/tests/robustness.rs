//! Robustness tests: corrupt the synthetic fixtures in many deterministic ways and check that
//! conversion never panics, never overflows the stack, finishes quickly, and never puts document
//! text in its report.
//!
//! These are not a replacement for fuzzing with a coverage-guided fuzzer (see `fuzz/`), but they
//! run on every CI build and every failure is reproducible from the printed case id.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ccda2fhir::{convert, Options};

/// Text that appears in the fixtures and must never reach a report.
const PHI_MARKERS: &[&str] = &[
    "Lee",
    "Example",
    "Doe",
    "MRN-",
    "1234567893",
    "Walk daily",
    "Penicillin",
    "Midstream",
    "Urine",
];

fn fixtures() -> Vec<(String, String)> {
    let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "xml"))
        .map(|e| {
            (
                e.path().file_stem().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(e.path()).unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

/// A tiny deterministic generator (xorshift), so cases are reproducible without a dependency.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % (n.max(1) as u64)).unwrap()
    }
}

/// Convert and check the invariants. `case` identifies the input without printing it.
fn check(case: &str, input: &[u8]) {
    let start = Instant::now();
    let result = convert(input, &Options::default());
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "{case}: conversion was too slow"
    );
    if let Ok(conversion) = result {
        // Output must serialize and every report entry must be free of document text.
        let json = serde_json::to_string(&conversion.bundle.to_json());
        assert!(json.is_ok(), "{case}: bundle did not serialize");
        for issue in &conversion.report.issues {
            let text = format!("{} {}", issue.code, issue.message);
            for marker in PHI_MARKERS {
                assert!(
                    !text.contains(marker),
                    "{case}: report entry {} contains document text",
                    issue.code
                );
            }
        }
    } else if let Err(e) = result {
        let text = e.to_string();
        for marker in PHI_MARKERS {
            assert!(
                !text.contains(marker),
                "{case}: error contains document text"
            );
        }
    }
}

#[test]
fn truncated_documents_do_not_panic() {
    for (name, xml) in fixtures() {
        let bytes = xml.as_bytes();
        for i in 0..=128 {
            let cut = bytes.len() * i / 128;
            check(&format!("{name}/truncate-{i}"), &bytes[..cut]);
        }
    }
}

#[test]
fn random_byte_damage_does_not_panic() {
    for (name, xml) in fixtures() {
        for seed in 1..=150u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let mut b = xml.clone().into_bytes();
            for _ in 0..=rng.below(6) {
                if b.is_empty() {
                    break;
                }
                let at = rng.below(b.len());
                match rng.below(3) {
                    0 => b[at] = b"<>&\"' =/"[rng.below(8)],
                    1 => {
                        let end = (at + 1 + rng.below(30)).min(b.len());
                        b.drain(at..end);
                    }
                    _ => {
                        let end = (at + 1 + rng.below(60)).min(b.len());
                        let chunk = b[at..end].to_vec();
                        let to = rng.below(b.len());
                        b.splice(to..to, chunk);
                    }
                }
            }
            check(&format!("{name}/damage-{seed}"), &b);
        }
    }
}

/// Replace attribute values with hostile ones while keeping the XML well formed.
#[test]
fn hostile_attribute_values_do_not_panic() {
    let values = [
        "",
        " ",
        "-",
        "0",
        "99999999999999999999999999999999999999",
        "-1e999",
        "NaN",
        "1.2.3.4.5.6.7.8.9.10.11.12.13.14.15.16.17.18",
        "２０２４０１０１",
        "20241301",
        "20240230",
        "2024",
        "20240101250000",
        "20240101120000+9999",
        "\u{202e}rtl",
        "a\u{0}b",
        "ÿ",
    ];
    for (name, xml) in fixtures() {
        for attr in [
            "code",
            "value",
            "codeSystem",
            "root",
            "extension",
            "unit",
            "nullFlavor",
        ] {
            for (i, v) in values.iter().enumerate() {
                let needle = format!(" {attr}=\"");
                let mut out = String::new();
                let mut rest = xml.as_str();
                while let Some(pos) = rest.find(&needle) {
                    let after = pos + needle.len();
                    out.push_str(&rest[..after]);
                    out.push_str(
                        &v.replace('&', "&amp;")
                            .replace('<', "&lt;")
                            .replace('"', "&quot;"),
                    );
                    let tail = &rest[after..];
                    let end = tail.find('"').unwrap_or(tail.len());
                    rest = &tail[end..];
                }
                out.push_str(rest);
                check(&format!("{name}/{attr}-{i}"), out.as_bytes());
            }
        }
    }
}

#[test]
fn repeated_sections_do_not_blow_up() {
    for (name, xml) in fixtures() {
        let (Some(start), Some(end)) =
            (xml.find("<structuredBody>"), xml.find("</structuredBody>"))
        else {
            continue;
        };
        let inner = &xml[start + "<structuredBody>".len()..end];
        let big = format!(
            "{}<structuredBody>{}</structuredBody>{}",
            &xml[..start],
            inner.repeat(200),
            &xml[end + "</structuredBody>".len()..]
        );
        check(&format!("{name}/repeat-200"), big.as_bytes());
    }
}

fn wrap(open: &str, close: &str, depth: usize) -> String {
    format!(
        r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><component><structuredBody><component><section><entry>{}{}</entry></section></component></structuredBody></component></ClinicalDocument>"#,
        open.repeat(depth),
        close.repeat(depth)
    )
}

#[test]
fn deep_nesting_is_an_error_not_a_stack_overflow() {
    for depth in [10, 100, 1_000, 10_000, 100_000] {
        let nested = wrap(
            r#"<observation classCode="OBS" moodCode="EVN"><entryRelationship typeCode="COMP">"#,
            "</entryRelationship></observation>",
            depth,
        );
        eprintln!("statements {depth}");
        check(&format!("nest-statements-{depth}"), nested.as_bytes());
        let sections = format!(
            r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><component><structuredBody>{}{}</structuredBody></component></ClinicalDocument>"#,
            "<component><section>".repeat(depth),
            "</section></component>".repeat(depth)
        );
        eprintln!("sections {depth}");
        check(&format!("nest-sections-{depth}"), sections.as_bytes());
        let plain = format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth));
        eprintln!("plain {depth}");
        check(&format!("nest-plain-{depth}"), plain.as_bytes());
    }
}

#[test]
fn huge_values_are_handled() {
    let big = "x".repeat(8 * 1024 * 1024);
    let xml = format!(
        r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><id root="{big}" extension="{big}"/><title>{big}</title></ClinicalDocument>"#
    );
    check("huge-attribute", xml.as_bytes());
    let many = format!(
        r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><component><structuredBody>{}</structuredBody></component></ClinicalDocument>"#,
        "<component><section/></component>".repeat(100_000)
    );
    check("many-empty-sections", many.as_bytes());
}

#[test]
fn hostile_markup_is_rejected_or_ignored() {
    let cases = [
        "",
        "\u{feff}",
        "not xml",
        "<?xml version=\"1.0\"?>",
        "<!DOCTYPE x [<!ENTITY a \"aaaaaaaaaa\"><!ENTITY b \"&a;&a;&a;&a;&a;&a;&a;&a;\">]><ClinicalDocument xmlns=\"urn:hl7-org:v3\">&b;</ClinicalDocument>",
        "<ClinicalDocument xmlns=\"urn:hl7-org:v3\"><![CDATA[x]]></ClinicalDocument>",
        "<ClinicalDocument xmlns=\"urn:other\"/>",
        "<ClinicalDocument/>",
        "<ClinicalDocument xmlns=\"urn:hl7-org:v3\"><?pi x?><!-- c --></ClinicalDocument>",
    ];
    for (i, c) in cases.iter().enumerate() {
        check(&format!("hostile-{i}"), c.as_bytes());
    }
    check("invalid-utf8", &[0xff, 0xfe, 0x3c, 0x61, 0x3e]);
}
