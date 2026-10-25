//! Data types shared by the header, sections and clinical statements.

use std::collections::BTreeMap;

use roxmltree::Node;

use crate::xml::{attr, child, children, is_hl7, text_of, XSI_NS};

/// `<templateId root=".." extension=".."/>`: names the rules a node conforms to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemplateId {
    pub root: String,
    /// Version, usually a date such as `2015-08-01`.
    pub extension: Option<String>,
}

/// An identifier: `<id root="..." extension="..." nullFlavor="..."/>`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Identifier {
    /// The OID or UUID that names the assigning namespace.
    pub root: Option<String>,
    /// The identifier value within that namespace.
    pub extension: Option<String>,
    /// HL7 `nullFlavor` if the identifier is intentionally absent, e.g. `NI`.
    pub null_flavor: Option<String>,
    /// `assigningAuthorityName`: a human readable name for the namespace.
    pub assigning_authority_name: Option<String>,
}

/// A coded value (`CD`/`CE`): `<code code=".." codeSystem=".." displayName=".."/>`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Code {
    /// The code itself.
    pub code: Option<String>,
    /// OID of the code system.
    pub code_system: Option<String>,
    /// Display name for the code.
    pub display_name: Option<String>,
    /// Text from `originalText`: either written there, or resolved from the section narrative.
    pub original_text: Option<String>,
    /// The `#reference` into the section narrative from `originalText`, if present.
    pub original_text_reference: Option<String>,
    /// HL7 `nullFlavor` if the code is intentionally absent.
    pub null_flavor: Option<String>,
    /// Alternative codings of the same concept.
    pub translations: Vec<Code>,
    /// `qualifier`s as (name, value) pairs, for example a DICOM series modality.
    pub qualifiers: Vec<(Code, Code)>,
}

/// One end of an interval, or a point in time. Raw HL7 `TS`, not yet normalized.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimePoint {
    pub value: Option<String>,
    pub null_flavor: Option<String>,
}

/// The `period` of a periodic interval (`PIVL_TS`): one value, or a `low`/`high` range.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimePeriod {
    pub value: Option<String>,
    pub unit: Option<String>,
    pub low: Option<Quantity>,
    pub high: Option<Quantity>,
}

/// An `effectiveTime`: a point (`value`), an interval (`low`/`high`), or, for medications, a
/// frequency (`PIVL_TS` with `period`, `EIVL_TS` with `event`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Time {
    pub value: Option<String>,
    pub low: Option<TimePoint>,
    pub high: Option<TimePoint>,
    pub null_flavor: Option<String>,
    /// The local name of `xsi:type`, e.g. `IVL_TS`, `PIVL_TS`, `EIVL_TS`.
    pub xsi_type: Option<String>,
    /// `operator`: `A` means this time intersects (further restricts) the previous one.
    pub operator: Option<String>,
    pub period: Option<TimePeriod>,
    /// `EIVL_TS` `event/@code` (for example `ACM`, `HS`).
    pub event: Option<String>,
    /// `EIVL_TS` `offset`.
    pub offset: Option<Quantity>,
}

/// A value with a raw string and an optional `nullFlavor`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scalar {
    pub value: Option<String>,
    pub null_flavor: Option<String>,
}

/// A physical quantity (`PQ`). The number stays a string so no precision is lost.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Quantity {
    pub value: Option<String>,
    /// UCUM unit.
    pub unit: Option<String>,
    pub null_flavor: Option<String>,
}

/// The typed `value` of an observation, chosen by its `xsi:type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// `CD`, `CE`, `CV`
    Coded(Code),
    /// `PQ`
    Quantity(Quantity),
    /// `INT`
    Integer(Scalar),
    /// `REAL`
    Real(Scalar),
    /// `ST`
    Text(Scalar),
    /// `BL`
    Boolean(Scalar),
    /// `TS`
    Timestamp(Scalar),
    /// `IVL_PQ`: a range of quantities.
    Range {
        low: Option<Quantity>,
        high: Option<Quantity>,
    },
    /// Any type we do not model yet. Kept so mappers can report it instead of silently losing it.
    Other { xsi_type: Option<String> },
}

/// `id` to text for every element in a section narrative that carries an `ID` attribute.
/// `BTreeMap` keeps iteration deterministic.
pub(crate) type Narrative = BTreeMap<String, String>;

pub(crate) fn template_ids(node: &Node) -> Vec<TemplateId> {
    children(node, "templateId")
        .filter_map(|n| {
            Some(TemplateId {
                root: attr(&n, "root")?,
                extension: attr(&n, "extension"),
            })
        })
        .collect()
}

pub(crate) fn identifier(node: &Node) -> Identifier {
    Identifier {
        root: attr(node, "root"),
        extension: attr(node, "extension"),
        null_flavor: attr(node, "nullFlavor"),
        assigning_authority_name: attr(node, "assigningAuthorityName"),
    }
}

pub(crate) fn identifiers(node: &Node) -> Vec<Identifier> {
    children(node, "id").map(|n| identifier(&n)).collect()
}

/// Look up a `#id` reference in the narrative.
fn resolve(reference: &str, narrative: &Narrative) -> Option<String> {
    narrative.get(reference.strip_prefix('#')?).cloned()
}

/// Parse a coded element and its `translation` children. `originalText` references are
/// resolved against `narrative` when the id is present there.
pub(crate) fn parse_code(node: &Node, narrative: &Narrative) -> Code {
    let original_text = child(node, "originalText");
    let reference = original_text
        .as_ref()
        .and_then(|n| child(n, "reference"))
        .and_then(|n| attr(&n, "value"));
    let own_text = original_text
        .as_ref()
        .filter(|n| child(n, "reference").is_none())
        .and_then(text_of);
    Code {
        code: attr(node, "code"),
        code_system: attr(node, "codeSystem"),
        display_name: attr(node, "displayName"),
        original_text: own_text
            .or_else(|| reference.as_deref().and_then(|r| resolve(r, narrative))),
        original_text_reference: reference,
        null_flavor: attr(node, "nullFlavor"),
        translations: children(node, "translation")
            .map(|c| parse_code(&c, narrative))
            .collect(),
        qualifiers: children(node, "qualifier")
            .filter_map(|q| {
                Some((
                    parse_code(&child(&q, "name")?, narrative),
                    parse_code(&child(&q, "value")?, narrative),
                ))
            })
            .collect(),
    }
}

fn time_point(node: &Node) -> TimePoint {
    TimePoint {
        value: attr(node, "value"),
        null_flavor: attr(node, "nullFlavor"),
    }
}

pub(crate) fn parse_quantity(node: &Node) -> Quantity {
    Quantity {
        value: attr(node, "value"),
        unit: attr(node, "unit"),
        null_flavor: attr(node, "nullFlavor"),
    }
}

pub(crate) fn parse_time(node: &Node) -> Time {
    let offset = child(node, "offset").map(|n| {
        // An offset may be a single quantity or a range; a range contributes its low end.
        child(&n, "low").map_or_else(|| parse_quantity(&n), |l| parse_quantity(&l))
    });
    Time {
        value: attr(node, "value"),
        low: child(node, "low").map(|n| time_point(&n)),
        high: child(node, "high").map(|n| time_point(&n)),
        null_flavor: attr(node, "nullFlavor"),
        xsi_type: node
            .attribute((XSI_NS, "type"))
            .map(|t| t.rsplit(':').next().unwrap_or(t).to_owned()),
        operator: attr(node, "operator"),
        period: child(node, "period").map(|p| TimePeriod {
            value: attr(&p, "value"),
            unit: attr(&p, "unit"),
            low: child(&p, "low").map(|n| parse_quantity(&n)),
            high: child(&p, "high").map(|n| parse_quantity(&n)),
        }),
        event: child(node, "event").and_then(|n| attr(&n, "code")),
        offset,
    }
}

fn scalar(node: &Node) -> Scalar {
    Scalar {
        value: attr(node, "value"),
        null_flavor: attr(node, "nullFlavor"),
    }
}

/// Parse `<value xsi:type="..."/>`.
pub(crate) fn parse_value(node: &Node, narrative: &Narrative) -> Value {
    let xsi_type = node.attribute((XSI_NS, "type"));
    // The prefix is irrelevant (`xsi:type="cda:CD"`); only the local name matters.
    let local = xsi_type.map(|t| t.rsplit(':').next().unwrap_or(t));
    match local {
        Some("CD" | "CE" | "CV" | "CO") => Value::Coded(parse_code(node, narrative)),
        Some("PQ") => Value::Quantity(Quantity {
            value: attr(node, "value"),
            unit: attr(node, "unit"),
            null_flavor: attr(node, "nullFlavor"),
        }),
        Some("IVL_PQ") => Value::Range {
            low: child(node, "low").map(|n| parse_quantity(&n)),
            high: child(node, "high").map(|n| parse_quantity(&n)),
        },
        Some("INT") => Value::Integer(scalar(node)),
        Some("REAL") => Value::Real(scalar(node)),
        Some("BL") => Value::Boolean(scalar(node)),
        Some("TS") => Value::Timestamp(scalar(node)),
        Some("ST") => Value::Text(Scalar {
            value: text_of(node),
            null_flavor: attr(node, "nullFlavor"),
        }),
        _ => Value::Other {
            xsi_type: xsi_type.map(str::to_owned),
        },
    }
}

/// True for the statement element names we recognize.
pub(crate) fn is_statement_element(node: &Node) -> bool {
    [
        "act",
        "observation",
        "substanceAdministration",
        "supply",
        "procedure",
        "encounter",
        "organizer",
    ]
    .iter()
    .any(|name| is_hl7(node, name))
}
