//! The document header: patient demographics (more header parts arrive with later phases).

use roxmltree::Node;

use crate::types::{identifiers, parse_code, Narrative};
use crate::xml::{attr, child, children, path, text_of};
use crate::{Code, Identifier};

/// A person name (`name`) with its parts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonName {
    /// `use` attribute (`L` legal, `P` pseudonym, ...).
    pub use_code: Option<String>,
    pub prefix: Vec<String>,
    pub given: Vec<String>,
    pub family: Option<String>,
    pub suffix: Vec<String>,
}

/// An address (`addr`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Address {
    pub use_code: Option<String>,
    pub lines: Vec<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
}

/// A telecom (`telecom`): `tel:`, `mailto:`, `fax:` or a URL.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Telecom {
    pub use_code: Option<String>,
    pub value: Option<String>,
}

/// `languageCommunication`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Language {
    pub code: Option<String>,
    pub preferred: Option<bool>,
}

/// A `guardian` of the patient.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Guardian {
    pub relationship: Option<Code>,
    pub names: Vec<PersonName>,
    pub addresses: Vec<Address>,
    pub telecoms: Vec<Telecom>,
}

/// The patient as described in the header's `recordTarget`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Patient {
    pub given: Vec<String>,
    pub family: Option<String>,
    /// Raw HL7 v3 timestamp, e.g. `19700101` (normalization happens in the mapper).
    pub birth_time: Option<String>,
    /// Administrative gender code, e.g. `M` or `F`.
    pub gender_code: Option<String>,
    /// Identifiers from `patientRole/id` (MRN, SSN and so on).
    pub identifiers: Vec<Identifier>,
    /// Every `name`, in document order (the first also fills `given` and `family`).
    pub names: Vec<PersonName>,
    pub addresses: Vec<Address>,
    pub telecoms: Vec<Telecom>,
    pub marital_status: Option<Code>,
    /// `raceCode` and `sdtc:raceCode`.
    pub race_codes: Vec<Code>,
    /// `ethnicGroupCode` and `sdtc:ethnicGroupCode`.
    pub ethnic_codes: Vec<Code>,
    pub languages: Vec<Language>,
    pub guardians: Vec<Guardian>,
}

fn parse_name(node: &Node) -> PersonName {
    let texts =
        |name: &str| -> Vec<String> { children(node, name).filter_map(|c| text_of(&c)).collect() };
    PersonName {
        use_code: attr(node, "use"),
        prefix: texts("prefix"),
        given: texts("given"),
        family: child(node, "family").and_then(|n| text_of(&n)),
        suffix: texts("suffix"),
    }
}

fn parse_address(node: &Node) -> Address {
    let text = |name: &str| child(node, name).and_then(|n| text_of(&n));
    Address {
        use_code: attr(node, "use"),
        lines: children(node, "streetAddressLine")
            .filter_map(|c| text_of(&c))
            .collect(),
        city: text("city"),
        state: text("state"),
        postal_code: text("postalCode"),
        country: text("country"),
    }
}

fn parse_telecom(node: &Node) -> Telecom {
    Telecom {
        use_code: attr(node, "use"),
        value: attr(node, "value"),
    }
}

/// Elements with this local name in any namespace (for `sdtc:` extensions).
fn local<'a, 'input>(
    node: &'a Node<'a, 'input>,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> + 'a {
    node.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == name)
}

pub(crate) fn parse_patient(root: &Node) -> Patient {
    let Some(role) = path(root, &["recordTarget", "patientRole"]) else {
        return Patient::default();
    };
    let identifiers = identifiers(&role);
    let Some(patient) = child(&role, "patient") else {
        return Patient {
            identifiers,
            addresses: children(&role, "addr").map(|n| parse_address(&n)).collect(),
            telecoms: children(&role, "telecom")
                .map(|n| parse_telecom(&n))
                .collect(),
            ..Patient::default()
        };
    };

    let names: Vec<PersonName> = children(&patient, "name").map(|n| parse_name(&n)).collect();
    let given = names.first().map(|n| n.given.clone()).unwrap_or_default();
    let family = names.first().and_then(|n| n.family.clone());
    let narrative = Narrative::new();
    let codes = |name: &str| -> Vec<Code> {
        local(&patient, name)
            .map(|n| parse_code(&n, &narrative))
            .collect()
    };

    Patient {
        given,
        family,
        birth_time: child(&patient, "birthTime").and_then(|n| attr(&n, "value")),
        gender_code: child(&patient, "administrativeGenderCode").and_then(|n| attr(&n, "code")),
        identifiers,
        names,
        addresses: children(&role, "addr").map(|n| parse_address(&n)).collect(),
        telecoms: children(&role, "telecom")
            .map(|n| parse_telecom(&n))
            .collect(),
        marital_status: child(&patient, "maritalStatusCode").map(|n| parse_code(&n, &narrative)),
        race_codes: codes("raceCode"),
        ethnic_codes: codes("ethnicGroupCode"),
        languages: children(&patient, "languageCommunication")
            .map(|l| Language {
                code: child(&l, "languageCode").and_then(|n| attr(&n, "code")),
                preferred: child(&l, "preferenceInd")
                    .and_then(|n| attr(&n, "value"))
                    .map(|v| v == "true"),
            })
            .collect(),
        guardians: children(&patient, "guardian")
            .map(|g| Guardian {
                relationship: child(&g, "code").map(|n| parse_code(&n, &narrative)),
                names: child(&g, "guardianPerson")
                    .map(|p| children(&p, "name").map(|n| parse_name(&n)).collect())
                    .unwrap_or_default(),
                addresses: children(&g, "addr").map(|n| parse_address(&n)).collect(),
                telecoms: children(&g, "telecom").map(|n| parse_telecom(&n)).collect(),
            })
            .collect(),
    }
}
