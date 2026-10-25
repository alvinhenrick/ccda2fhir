//! The document body: sections, entries and clinical statements.
//!
//! A *clinical statement* is one of the C-CDA building blocks (`act`, `observation`,
//! `substanceAdministration`, `supply`, `procedure`, `encounter`, `organizer`). They nest through
//! `entryRelationship` (and `component` for organizers). The model is generic on purpose: which
//! *kind* of thing a statement is comes from its `templateId`, and that decision belongs to the
//! mappers, not the parser.

use roxmltree::Node;

use crate::types::{
    identifiers, is_statement_element, parse_code, parse_time, parse_value, template_ids, Narrative,
};
use crate::xml::{attr, child, children, is_hl7, text_of};
use crate::{Code, Identifier, ParseError, TemplateId, Time, Value};

/// How deep statements may nest before the parser gives up. Real documents stay far below this.
pub(crate) const MAX_DEPTH: usize = 32;

/// Which clinical statement element this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementKind {
    Act,
    Observation,
    SubstanceAdministration,
    Supply,
    Procedure,
    Encounter,
    Organizer,
}

impl StatementKind {
    fn from_element(node: &Node) -> Option<Self> {
        Some(match node.tag_name().name() {
            "act" => Self::Act,
            "observation" => Self::Observation,
            "substanceAdministration" => Self::SubstanceAdministration,
            "supply" => Self::Supply,
            "procedure" => Self::Procedure,
            "encounter" => Self::Encounter,
            "organizer" => Self::Organizer,
            _ => return None,
        })
    }
}

/// How a nested statement is attached to its parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// `entryRelationship`, with its `typeCode` (`SUBJ`, `COMP`, `RSON`, `SPRT`, ...) and
    /// `inversionInd`.
    EntryRelationship {
        type_code: Option<String>,
        inversion_ind: Option<bool>,
    },
    /// `component` (used by organizers).
    Component,
}

/// A statement nested in another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Related {
    pub link: Link,
    pub statement: Statement,
}

/// A clinical statement and everything nested under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub kind: StatementKind,
    pub class_code: Option<String>,
    pub mood_code: Option<String>,
    /// `negationInd="true"`: the statement says the thing did *not* happen / is not present.
    pub negation_ind: bool,
    pub template_ids: Vec<TemplateId>,
    pub ids: Vec<Identifier>,
    pub code: Option<Code>,
    /// The `text` element: written text, or resolved from the section narrative.
    pub text: Option<String>,
    pub status_code: Option<String>,
    pub effective_time: Option<Time>,
    /// `value` elements (observations normally have one).
    pub values: Vec<Value>,
    pub related: Vec<Related>,
}

impl Statement {
    /// True if the statement declares the given template root (any version).
    #[must_use]
    pub fn has_template(&self, root: &str) -> bool {
        self.template_ids.iter().any(|t| t.root == root)
    }

    /// Nested statements attached with the given `entryRelationship` `typeCode`.
    pub fn related_by<'a>(&'a self, type_code: &'a str) -> impl Iterator<Item = &'a Statement> {
        self.related.iter().filter_map(move |r| match &r.link {
            Link::EntryRelationship {
                type_code: Some(t), ..
            } if t == type_code => Some(&r.statement),
            _ => None,
        })
    }
}

/// An `entry` of a section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub type_code: Option<String>,
    pub statement: Statement,
}

/// A section, with its entries and any subsections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub template_ids: Vec<TemplateId>,
    pub code: Option<Code>,
    pub title: Option<String>,
    /// `nullFlavor` on the section (for example `NI`: "no information").
    pub null_flavor: Option<String>,
    /// The narrative `text`, flattened to plain text.
    pub text: Option<String>,
    pub entries: Vec<Entry>,
    pub sections: Vec<Section>,
}

impl Section {
    /// True if the section declares the given template root (any version).
    #[must_use]
    pub fn has_template(&self, root: &str) -> bool {
        self.template_ids.iter().any(|t| t.root == root)
    }
}

/// Collect `ID` to text for every element inside the section's `text`.
fn narrative_of(section: &Node) -> Narrative {
    let mut map = Narrative::new();
    if let Some(text) = child(section, "text") {
        for node in text.descendants().filter(Node::is_element) {
            if let (Some(id), Some(value)) = (attr(&node, "ID"), text_of(&node)) {
                map.entry(id).or_insert(value);
            }
        }
    }
    map
}

fn parse_statement(
    node: &Node,
    narrative: &Narrative,
    depth: usize,
) -> Result<Statement, ParseError> {
    if depth > MAX_DEPTH {
        return Err(ParseError::TooDeep);
    }
    let kind = StatementKind::from_element(node).ok_or(ParseError::Xml)?;

    let mut related = Vec::new();
    for rel in node.children().filter(Node::is_element) {
        let link = if is_hl7(&rel, "entryRelationship") {
            Link::EntryRelationship {
                type_code: attr(&rel, "typeCode"),
                inversion_ind: attr(&rel, "inversionInd").map(|v| v == "true"),
            }
        } else if is_hl7(&rel, "component") && kind == StatementKind::Organizer {
            Link::Component
        } else {
            continue;
        };
        if let Some(inner) = rel.children().find(is_statement_element) {
            related.push(Related {
                link,
                statement: parse_statement(&inner, narrative, depth + 1)?,
            });
        }
    }

    let text = child(node, "text").and_then(|n| {
        child(&n, "reference")
            .and_then(|r| attr(&r, "value"))
            .and_then(|r| narrative.get(r.strip_prefix('#')?).cloned())
            .or_else(|| text_of(&n))
    });

    Ok(Statement {
        kind,
        class_code: attr(node, "classCode"),
        mood_code: attr(node, "moodCode"),
        negation_ind: node.attribute("negationInd") == Some("true"),
        template_ids: template_ids(node),
        ids: identifiers(node),
        code: child(node, "code").map(|n| parse_code(&n, narrative)),
        text,
        status_code: child(node, "statusCode").and_then(|n| attr(&n, "code")),
        effective_time: child(node, "effectiveTime").map(|n| parse_time(&n)),
        values: children(node, "value")
            .map(|n| parse_value(&n, narrative))
            .collect(),
        related,
    })
}

fn parse_section(node: &Node, depth: usize) -> Result<Section, ParseError> {
    if depth > MAX_DEPTH {
        return Err(ParseError::TooDeep);
    }
    let narrative = narrative_of(node);

    let mut entries = Vec::new();
    for entry in children(node, "entry") {
        if let Some(inner) = entry.children().find(is_statement_element) {
            entries.push(Entry {
                type_code: attr(&entry, "typeCode"),
                statement: parse_statement(&inner, &narrative, 0)?,
            });
        }
    }

    let mut sections = Vec::new();
    for component in children(node, "component") {
        if let Some(inner) = child(&component, "section") {
            sections.push(parse_section(&inner, depth + 1)?);
        }
    }

    Ok(Section {
        template_ids: template_ids(node),
        code: child(node, "code").map(|n| parse_code(&n, &narrative)),
        title: child(node, "title").and_then(|n| text_of(&n)),
        null_flavor: attr(node, "nullFlavor"),
        text: child(node, "text").and_then(|n| text_of(&n)),
        entries,
        sections,
    })
}

/// Parse `component/structuredBody/component/section`.
pub(crate) fn parse_body(root: &Node) -> Result<Vec<Section>, ParseError> {
    let Some(body) = child(root, "component").and_then(|c| child(&c, "structuredBody")) else {
        return Ok(Vec::new());
    };
    let mut sections = Vec::new();
    for component in children(&body, "component") {
        if let Some(section) = child(&component, "section") {
            sections.push(parse_section(&section, 0)?);
        }
    }
    Ok(sections)
}
