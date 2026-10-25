//! The document header: patient demographics (more header parts arrive with later phases).

use roxmltree::Node;

use crate::types::identifiers;
use crate::xml::{attr, child, children, path, text_of};
use crate::Identifier;

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
}

pub(crate) fn parse_patient(root: &Node) -> Patient {
    let Some(role) = path(root, &["recordTarget", "patientRole"]) else {
        return Patient::default();
    };
    let identifiers = identifiers(&role);
    let Some(patient) = child(&role, "patient") else {
        return Patient {
            identifiers,
            ..Patient::default()
        };
    };

    let name = child(&patient, "name");
    let given = name
        .iter()
        .flat_map(|n| children(n, "given"))
        .filter_map(|c| text_of(&c))
        .collect();
    let family = name
        .and_then(|n| child(&n, "family"))
        .and_then(|n| text_of(&n));

    Patient {
        given,
        family,
        birth_time: child(&patient, "birthTime").and_then(|n| attr(&n, "value")),
        gender_code: child(&patient, "administrativeGenderCode").and_then(|n| attr(&n, "code")),
        identifiers,
    }
}
