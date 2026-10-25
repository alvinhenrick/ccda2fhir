//! The document body: sections, entries and clinical statements.
//!
//! A *clinical statement* is one of the C-CDA building blocks (`act`, `observation`,
//! `substanceAdministration`, `supply`, `procedure`, `encounter`, `organizer`). They nest through
//! `entryRelationship` (and `component` for organizers). The model is generic on purpose: which
//! *kind* of thing a statement is comes from its `templateId`, and that decision belongs to the
//! mappers, not the parser.

use roxmltree::Node;

use crate::types::{
    identifiers, is_statement_element, parse_code, parse_quantity, parse_time, parse_value,
    template_ids, Narrative,
};
use crate::xml::{attr, child, children, is_hl7, text_of};
use crate::{Address, Code, Identifier, ParseError, Quantity, Telecom, TemplateId, Time, Value};

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

/// An organization named by an author (`representedOrganization`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthorOrganization {
    pub ids: Vec<Identifier>,
    pub name: Option<String>,
    pub telecoms: Vec<Telecom>,
    pub addresses: Vec<Address>,
}

/// Who acted on a statement: an `author` (when `time` is the authoring time) or a `performer`
/// (`assignedEntity`; `time` is the start of `performer/time`, if any).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Author {
    /// `author/time`, raw HL7 timestamp.
    pub time: Option<String>,
    /// `assignedAuthor/id`
    pub ids: Vec<Identifier>,
    /// `assignedAuthor/code` (role or specialty)
    pub role_code: Option<Code>,
    pub given: Vec<String>,
    pub family: Option<String>,
    /// `assignedAuthoringDevice/softwareName`: the author is software, not a person.
    pub device_name: Option<String>,
    pub organization: Option<AuthorOrganization>,
    /// `telecom` and `addr` of the assigned/associated/related entity.
    pub telecoms: Vec<Telecom>,
    pub addresses: Vec<Address>,
    /// `typeCode` of the header element (for a `participant`: `IND`, `HLD`, ...).
    pub type_code: Option<String>,
    /// `classCode` of an `associatedEntity` (`NOK`, `ECON`, `CAREGIVER`, ...).
    pub class_code: Option<String>,
    /// `signatureCode/@code` of an authenticator.
    pub signature_code: Option<String>,
    /// The party is a `relatedEntity` or `associatedEntity`: a person related to the patient,
    /// not a care provider.
    pub is_related_person: bool,
}

/// A `relatedDocument` of the header: this document replaces, appends to or transforms another.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelatedDocument {
    /// `RPLC`, `APND`, `XFRM` (or `SIGN`).
    pub type_code: Option<String>,
    /// `parentDocument/id`
    pub ids: Vec<Identifier>,
}

/// A header party: `legalAuthenticator`, `authenticator`, `dataEnterer`, `informant` or
/// `participant`, whichever of `assignedEntity`, `relatedEntity` or `associatedEntity` it has.
pub(crate) fn parse_header_party(node: &Node, narrative: &Narrative) -> Author {
    let time = child(node, "time").and_then(|n| attr(&n, "value"));
    let entity = ["assignedEntity", "relatedEntity", "associatedEntity"]
        .iter()
        .find_map(|n| child(node, n));
    let mut a = parse_agent(entity, time, narrative);
    a.is_related_person = child(node, "assignedEntity").is_none() && entity.is_some();
    a.type_code = attr(node, "typeCode");
    a.class_code = entity.and_then(|e| attr(&e, "classCode"));
    a.signature_code = child(node, "signatureCode").and_then(|n| attr(&n, "code"));
    a
}

pub(crate) fn parse_related_document(node: &Node) -> RelatedDocument {
    RelatedDocument {
        type_code: attr(node, "typeCode"),
        ids: child(node, "parentDocument")
            .map(|p| identifiers(&p))
            .unwrap_or_default(),
    }
}

/// The `subject/relatedSubject` of a statement: a person related to the patient (a family member).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelatedSubject {
    /// `relatedSubject/code`: the relationship to the patient (HL7 RoleCode, for example `FTH`).
    pub code: Option<Code>,
    /// `sdtc:id` of the person.
    pub ids: Vec<Identifier>,
    pub name: Option<String>,
    /// `administrativeGenderCode/@code`
    pub gender_code: Option<String>,
    /// `birthTime/@value`
    pub birth_time: Option<String>,
    /// `sdtc:deceasedInd/@value`
    pub deceased_ind: Option<bool>,
    /// `sdtc:deceasedTime/@value`
    pub deceased_time: Option<String>,
}

fn parse_related_subject(node: &Node, narrative: &Narrative) -> Option<RelatedSubject> {
    let related = child(node, "subject").and_then(|s| child(&s, "relatedSubject"))?;
    let person = child(&related, "subject");
    // The extension namespace (`sdtc:`) is not HL7 v3, so match on the local name only.
    let ext = |name: &str| -> Option<Node> {
        person.and_then(|p| {
            p.children()
                .find(|c| c.is_element() && c.tag_name().name() == name)
        })
    };
    Some(RelatedSubject {
        code: child(&related, "code").map(|n| parse_code(&n, narrative)),
        ids: person
            .iter()
            .flat_map(|p| {
                p.children()
                    .filter(|c| c.is_element() && c.tag_name().name() == "id")
            })
            .map(|n| crate::types::identifier(&n))
            .collect(),
        name: person.and_then(|p| child(&p, "name")).and_then(|n| {
            let given: Vec<String> = children(&n, "given").filter_map(|g| text_of(&g)).collect();
            let family = child(&n, "family").and_then(|f| text_of(&f));
            if given.is_empty() && family.is_none() {
                text_of(&n)
            } else {
                Some(
                    given
                        .into_iter()
                        .chain(family)
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            }
        }),
        gender_code: person
            .and_then(|p| child(&p, "administrativeGenderCode"))
            .and_then(|n| attr(&n, "code")),
        birth_time: person
            .and_then(|p| child(&p, "birthTime"))
            .and_then(|n| attr(&n, "value")),
        deceased_ind: ext("deceasedInd")
            .and_then(|n| attr(&n, "value"))
            .map(|v| v == "true"),
        deceased_time: ext("deceasedTime").and_then(|n| attr(&n, "value")),
    })
}

/// The product of a `consumable` (a medication or vaccine).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Consumable {
    /// Templates on `manufacturedProduct` (for example Immunization Medication Information).
    pub template_ids: Vec<TemplateId>,
    /// `manufacturedMaterial/code`
    pub code: Option<Code>,
    /// `manufacturedMaterial/name`
    pub name: Option<String>,
    /// `manufacturedMaterial/lotNumberText`
    pub lot_number: Option<String>,
    /// `manufacturerOrganization`
    pub manufacturer: Option<AuthorOrganization>,
}

/// A `participant` (for example the allergen of an allergy, or a device).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Participant {
    /// `typeCode`: `CSM` (consumable), `PRD` (product), `LOC` (location), ...
    pub type_code: Option<String>,
    pub template_ids: Vec<TemplateId>,
    /// `participantRole/id`
    pub ids: Vec<Identifier>,
    /// `participantRole/code`
    pub role_code: Option<Code>,
    /// `participantRole/playingEntity/code`
    pub playing_entity_code: Option<Code>,
    /// `participantRole/playingDevice/code`
    pub playing_device_code: Option<Code>,
    /// `participantRole/playingEntity/name` (for a location, its name).
    pub name: Option<String>,
    /// The same name, split into parts (for a person).
    pub person_name: Option<crate::PersonName>,
    /// `participantRole/addr`
    pub addresses: Vec<Address>,
    /// `participantRole/telecom`
    pub telecoms: Vec<Telecom>,
}

/// A `specimen` of a result: `specimenRole` and its `specimenPlayingEntity`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Specimen {
    /// `specimenRole/id`
    pub ids: Vec<Identifier>,
    /// `specimenPlayingEntity/code`
    pub code: Option<Code>,
    /// `specimenPlayingEntity/name`
    pub name: Option<String>,
    /// `specimenPlayingEntity/desc`
    pub description: Option<String>,
    /// `specimenPlayingEntity/quantity`
    pub quantity: Option<Quantity>,
}

/// `referenceRange/observationRange` of an observation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReferenceRange {
    /// `interpretationCode` (`N` normal, `H`, `L`, ...).
    pub interpretation: Option<Code>,
    pub text: Option<String>,
    pub value: Option<Value>,
}

/// The `text` element of a statement when it carries its own content (a note's body).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextContent {
    /// `mediaType`, for example `application/rtf` or `text/plain`.
    pub media_type: Option<String>,
    /// `representation`: `B64` or `TXT`.
    pub representation: Option<String>,
    /// The element's own text, with no narrative reference resolved into it.
    pub content: Option<String>,
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
    /// The `text` element's own media type, representation and content.
    pub text_content: Option<TextContent>,
    pub status_code: Option<String>,
    pub effective_time: Option<Time>,
    /// Every `effectiveTime` in document order. Medications carry the validity interval first
    /// and the frequency (`PIVL_TS`/`EIVL_TS`, `operator="A"`) after it.
    pub effective_times: Vec<Time>,
    /// `value` elements (observations normally have one).
    pub values: Vec<Value>,
    pub participants: Vec<Participant>,
    /// `subject/relatedSubject`, for family history organizers.
    pub subject: Option<RelatedSubject>,
    pub specimens: Vec<Specimen>,
    pub interpretation_codes: Vec<Code>,
    pub method_codes: Vec<Code>,
    pub target_site_codes: Vec<Code>,
    pub reference_ranges: Vec<ReferenceRange>,
    pub authors: Vec<Author>,
    pub performers: Vec<Author>,
    /// `repeatNumber/@value` (dose number for immunizations, repeats for orders).
    pub repeat_number: Option<String>,
    pub route_code: Option<Code>,
    /// `priorityCode` (ActPriority: `R` routine, `UR` urgent, ...).
    pub priority_code: Option<Code>,
    pub approach_site_code: Option<Code>,
    /// `sdtc:dischargeDispositionCode` of an encounter.
    pub discharge_disposition: Option<Code>,
    /// `doseQuantity` when it is a plain quantity.
    pub dose_quantity: Option<Quantity>,
    /// `doseQuantity` when it is a `low`/`high` interval.
    pub dose_range: Option<(Option<Quantity>, Option<Quantity>)>,
    pub rate_quantity: Option<Quantity>,
    /// `maxDoseQuantity`: numerator and denominator.
    pub max_dose_quantity: Option<(Option<Quantity>, Option<Quantity>)>,
    /// `quantity` (supply orders and dispenses).
    pub quantity: Option<Quantity>,
    /// A `precondition` is present (for example "as needed").
    pub has_precondition: bool,
    pub administration_unit_code: Option<Code>,
    pub consumable: Option<Consumable>,
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

pub(crate) fn parse_author(node: &Node, narrative: &Narrative) -> Author {
    let time = child(node, "time").and_then(|n| attr(&n, "value"));
    parse_agent(child(node, "assignedAuthor"), time, narrative)
}

/// An `assignedAuthor` or `assignedEntity`: ids, role, person name, device, organization.
fn parse_agent(assigned: Option<Node>, time: Option<String>, narrative: &Narrative) -> Author {
    let person = assigned.and_then(|a| {
        ["assignedPerson", "associatedPerson", "relatedPerson"]
            .iter()
            .find_map(|n| child(&a, n))
    });
    let name = person.and_then(|p| child(&p, "name"));
    let organization = assigned
        .and_then(|a| child(&a, "representedOrganization"))
        .map(|o| parse_organization(&o));
    Author {
        time,
        ids: assigned.as_ref().map(identifiers).unwrap_or_default(),
        role_code: assigned
            .and_then(|a| child(&a, "code"))
            .map(|n| parse_code(&n, narrative)),
        given: name
            .iter()
            .flat_map(|n| children(n, "given"))
            .filter_map(|n| text_of(&n))
            .collect(),
        family: name
            .and_then(|n| child(&n, "family"))
            .and_then(|n| text_of(&n)),
        device_name: assigned
            .and_then(|a| child(&a, "assignedAuthoringDevice"))
            .and_then(|d| child(&d, "softwareName"))
            .and_then(|n| text_of(&n)),
        organization,
        telecoms: assigned
            .iter()
            .flat_map(|a| children(a, "telecom"))
            .map(|n| crate::header::parse_telecom(&n))
            .collect(),
        addresses: assigned
            .iter()
            .flat_map(|a| children(a, "addr"))
            .map(|n| crate::header::parse_address(&n))
            .collect(),
        type_code: None,
        class_code: None,
        signature_code: None,
        is_related_person: false,
    }
}

pub(crate) fn parse_organization(node: &Node) -> AuthorOrganization {
    AuthorOrganization {
        ids: identifiers(node),
        name: child(node, "name").and_then(|n| text_of(&n)),
        telecoms: children(node, "telecom")
            .map(|n| crate::header::parse_telecom(&n))
            .collect(),
        addresses: children(node, "addr")
            .map(|n| crate::header::parse_address(&n))
            .collect(),
    }
}

fn parse_consumable(node: &Node, narrative: &Narrative) -> Option<Consumable> {
    let product = child(node, "manufacturedProduct")?;
    let material = child(&product, "manufacturedMaterial");
    Some(Consumable {
        template_ids: template_ids(&product),
        code: material
            .and_then(|m| child(&m, "code"))
            .map(|n| parse_code(&n, narrative)),
        name: material
            .and_then(|m| child(&m, "name"))
            .and_then(|n| text_of(&n)),
        lot_number: material
            .and_then(|m| child(&m, "lotNumberText"))
            .and_then(|n| text_of(&n)),
        manufacturer: child(&product, "manufacturerOrganization").map(|o| parse_organization(&o)),
    })
}

fn parse_participant(node: &Node, narrative: &Narrative) -> Participant {
    let role = child(node, "participantRole");
    let code_of = |parent: Option<Node>, name: &str| {
        parent
            .and_then(|p| child(&p, name))
            .and_then(|n| child(&n, "code"))
            .map(|n| parse_code(&n, narrative))
    };
    Participant {
        type_code: attr(node, "typeCode"),
        // C-CDA puts the templateId on `participantRole`; accept it on `participant` too.
        template_ids: template_ids(node)
            .into_iter()
            .chain(role.as_ref().map(template_ids).unwrap_or_default())
            .collect(),
        ids: role.as_ref().map(identifiers).unwrap_or_default(),
        role_code: role
            .and_then(|r| child(&r, "code"))
            .map(|n| parse_code(&n, narrative)),
        playing_entity_code: code_of(role, "playingEntity"),
        playing_device_code: code_of(role, "playingDevice"),
        name: role
            .and_then(|r| child(&r, "playingEntity"))
            .and_then(|e| child(&e, "name"))
            .and_then(|n| text_of(&n)),
        person_name: role
            .and_then(|r| child(&r, "playingEntity"))
            .and_then(|e| child(&e, "name"))
            .map(|n| crate::header::parse_name(&n)),
        addresses: role
            .iter()
            .flat_map(|r| children(r, "addr"))
            .map(|n| crate::header::parse_address(&n))
            .collect(),
        telecoms: role
            .iter()
            .flat_map(|r| children(r, "telecom"))
            .map(|n| crate::header::parse_telecom(&n))
            .collect(),
    }
}

fn parse_specimen(node: &Node, narrative: &Narrative) -> Specimen {
    let role = child(node, "specimenRole");
    let entity = role.and_then(|r| child(&r, "specimenPlayingEntity"));
    Specimen {
        ids: role.as_ref().map(identifiers).unwrap_or_default(),
        code: entity
            .and_then(|e| child(&e, "code"))
            .map(|n| parse_code(&n, narrative)),
        name: entity
            .and_then(|e| child(&e, "name"))
            .and_then(|n| text_of(&n)),
        description: entity
            .and_then(|e| child(&e, "desc"))
            .and_then(|n| text_of(&n)),
        quantity: entity
            .and_then(|e| child(&e, "quantity"))
            .map(|n| crate::types::parse_quantity(&n)),
    }
}

fn parse_statement(
    node: &Node,
    narrative: &Narrative,
    depth: usize,
) -> Result<Statement, ParseError> {
    let kind = StatementKind::from_element(node).ok_or(ParseError::Xml)?;
    parse_statement_as(node, narrative, depth, kind)
}

/// A header `documentationOf/serviceEvent`: the care the document describes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceEvent {
    pub code: Option<Code>,
    pub effective_time: Option<Time>,
    /// The `performer`s (for example the primary care provider), with their `functionCode`.
    pub performers: Vec<Author>,
}

pub(crate) fn parse_service_event(node: &Node, narrative: &Narrative) -> ServiceEvent {
    ServiceEvent {
        code: child(node, "code").map(|n| parse_code(&n, narrative)),
        effective_time: child(node, "effectiveTime").map(|n| parse_time(&n)),
        performers: children(node, "performer")
            .map(|n| {
                let time = child(&n, "time").and_then(|t| {
                    attr(&t, "value").or_else(|| child(&t, "low").and_then(|l| attr(&l, "value")))
                });
                let mut agent = parse_agent(child(&n, "assignedEntity"), time, narrative);
                // The performer's function (PCP, ...) is more specific than the entity's code.
                if let Some(f) = child(&n, "functionCode") {
                    agent.role_code = Some(parse_code(&f, narrative));
                }
                agent
            })
            .collect(),
    }
}

/// The header's `componentOf/encompassingEncounter` as an Encounter statement. Its performers
/// are the `encounterParticipant`s and its location the `healthCareFacility`.
pub(crate) fn parse_encompassing_encounter(
    node: &Node,
    narrative: &Narrative,
) -> Option<Statement> {
    let mut s = parse_statement_as(node, narrative, 0, StatementKind::Encounter).ok()?;
    s.mood_code.get_or_insert_with(|| "EVN".to_owned());
    s.performers = children(node, "encounterParticipant")
        .map(|n| {
            let time = child(&n, "time").and_then(|t| {
                attr(&t, "value").or_else(|| child(&t, "low").and_then(|l| attr(&l, "value")))
            });
            parse_agent(child(&n, "assignedEntity"), time, narrative)
        })
        .collect();
    if let Some(facility) = child(node, "location").and_then(|l| child(&l, "healthCareFacility")) {
        let place = child(&facility, "location");
        s.participants.push(Participant {
            type_code: Some("LOC".to_owned()),
            ids: identifiers(&facility),
            role_code: child(&facility, "code").map(|n| parse_code(&n, narrative)),
            name: place
                .and_then(|p| child(&p, "name"))
                .and_then(|n| text_of(&n)),
            addresses: place
                .iter()
                .flat_map(|p| children(p, "addr"))
                .map(|n| crate::header::parse_address(&n))
                .collect(),
            ..Participant::default()
        });
    }
    Some(s)
}

fn parse_statement_as(
    node: &Node,
    narrative: &Narrative,
    depth: usize,
    kind: StatementKind,
) -> Result<Statement, ParseError> {
    if depth > MAX_DEPTH {
        return Err(ParseError::TooDeep);
    }

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
        text_content: child(node, "text").map(|n| TextContent {
            media_type: attr(&n, "mediaType"),
            representation: attr(&n, "representation"),
            content: n
                .children()
                .filter(|c| c.is_text())
                .filter_map(|c| c.text())
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .reduce(|a, b| a + &b),
        }),
        status_code: child(node, "statusCode").and_then(|n| attr(&n, "code")),
        effective_time: child(node, "effectiveTime").map(|n| parse_time(&n)),
        effective_times: children(node, "effectiveTime")
            .map(|n| parse_time(&n))
            .collect(),
        rate_quantity: child(node, "rateQuantity").map(|n| parse_quantity(&n)),
        max_dose_quantity: child(node, "maxDoseQuantity").map(|n| {
            (
                child(&n, "numerator").map(|q| parse_quantity(&q)),
                child(&n, "denominator").map(|q| parse_quantity(&q)),
            )
        }),
        quantity: child(node, "quantity").map(|n| parse_quantity(&n)),
        has_precondition: child(node, "precondition").is_some(),
        values: children(node, "value")
            .map(|n| parse_value(&n, narrative))
            .collect(),
        performers: children(node, "performer")
            .map(|n| {
                let time = child(&n, "time").and_then(|t| {
                    attr(&t, "value").or_else(|| child(&t, "low").and_then(|l| attr(&l, "value")))
                });
                parse_agent(child(&n, "assignedEntity"), time, narrative)
            })
            .collect(),
        repeat_number: child(node, "repeatNumber").and_then(|n| attr(&n, "value")),
        route_code: child(node, "routeCode").map(|n| parse_code(&n, narrative)),
        priority_code: child(node, "priorityCode").map(|n| parse_code(&n, narrative)),
        approach_site_code: child(node, "approachSiteCode").map(|n| parse_code(&n, narrative)),
        // The extension namespace (`sdtc:`) is not HL7 v3, so match on the local name only.
        discharge_disposition: node
            .children()
            .find(|c| c.is_element() && c.tag_name().name() == "dischargeDispositionCode")
            .map(|n| parse_code(&n, narrative)),
        dose_quantity: child(node, "doseQuantity")
            .filter(|n| child(n, "low").is_none() && child(n, "high").is_none())
            .map(|n| parse_quantity(&n)),
        dose_range: child(node, "doseQuantity")
            .filter(|n| child(n, "low").is_some() || child(n, "high").is_some())
            .map(|n| {
                (
                    child(&n, "low").map(|q| parse_quantity(&q)),
                    child(&n, "high").map(|q| parse_quantity(&q)),
                )
            }),
        administration_unit_code: child(node, "administrationUnitCode")
            .map(|n| parse_code(&n, narrative)),
        consumable: child(node, "consumable")
            .or_else(|| child(node, "product"))
            .and_then(|n| parse_consumable(&n, narrative)),
        interpretation_codes: children(node, "interpretationCode")
            .map(|n| parse_code(&n, narrative))
            .collect(),
        method_codes: children(node, "methodCode")
            .map(|n| parse_code(&n, narrative))
            .collect(),
        target_site_codes: children(node, "targetSiteCode")
            .map(|n| parse_code(&n, narrative))
            .collect(),
        reference_ranges: children(node, "referenceRange")
            .filter_map(|n| child(&n, "observationRange"))
            .map(|r| ReferenceRange {
                interpretation: child(&r, "interpretationCode").map(|n| parse_code(&n, narrative)),
                text: child(&r, "text").and_then(|n| text_of(&n)),
                value: child(&r, "value").map(|n| parse_value(&n, narrative)),
            })
            .collect(),
        authors: children(node, "author")
            .map(|n| parse_author(&n, narrative))
            .collect(),
        participants: children(node, "participant")
            .map(|n| parse_participant(&n, narrative))
            .collect(),
        subject: parse_related_subject(node, narrative),
        specimens: children(node, "specimen")
            .map(|n| parse_specimen(&n, narrative))
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

/// The body of an unstructured document (`component/nonXMLBody/text`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NonXmlBody {
    /// `mediaType`, for example `application/pdf`.
    pub media_type: Option<String>,
    /// `representation`: `B64` or `TXT`.
    pub representation: Option<String>,
    /// The element's own text (base64 or plain), when it has any.
    pub content: Option<String>,
    /// `text/reference/@value`: a pointer to a file outside the document.
    pub reference: Option<String>,
}

/// Parse `component/nonXMLBody`, if there is one.
pub(crate) fn parse_non_xml_body(root: &Node) -> Option<NonXmlBody> {
    let body = child(root, "component").and_then(|c| child(&c, "nonXMLBody"))?;
    let text = child(&body, "text");
    Some(NonXmlBody {
        media_type: text.as_ref().and_then(|n| attr(n, "mediaType")),
        representation: text.as_ref().and_then(|n| attr(n, "representation")),
        content: text.as_ref().and_then(text_of),
        reference: text
            .as_ref()
            .and_then(|n| child(n, "reference"))
            .and_then(|n| attr(&n, "value")),
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
