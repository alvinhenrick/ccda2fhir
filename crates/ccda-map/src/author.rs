//! C-CDA authors to `Practitioner`, `Organization`, `recorder`, `recordedDate` and `Provenance`.
//!
//! The IG does not say when to use attributes versus Provenance resources, so this crate does
//! both (ADR 0004): `recorder` is the latest author, `recordedDate` the earliest author time,
//! each distinct person and organization becomes one resource, and every target with usable
//! authors gets a US Core `Provenance`.

use ccda_model::{Author, AuthorOrganization};
use serde_json::{json, Map, Value};

use crate::{map_codeable_concept, map_datetime, map_identifier, Context};

const PARTICIPANT_TYPE_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/provenance-participant-type";

/// A sortable key for an HL7 timestamp: its leading digits padded to 14. Offsets are ignored,
/// which is accurate enough to order authors within one document.
fn time_key(raw: &str) -> String {
    let digits: String = raw.chars().take_while(char::is_ascii_digit).collect();
    format!("{digits:0<14}")
}

/// The identity key of an author's identifiers or, failing that, name.
fn person_key(author: &Author) -> Option<String> {
    // Software is not a person, even when it has an id.
    if author.device_name.is_some() && author.family.is_none() && author.given.is_empty() {
        return None;
    }
    author
        .ids
        .iter()
        .find_map(|id| {
            Some(format!(
                "id:{}|{}",
                id.root.as_deref()?,
                id.extension.as_deref()?
            ))
        })
        .or_else(|| {
            (author.family.is_some() || !author.given.is_empty()).then(|| {
                format!(
                    "name:{}|{}",
                    author.given.join(" "),
                    author.family.as_deref().unwrap_or_default()
                )
            })
        })
}

pub(crate) fn register_organization(org: &AuthorOrganization, ctx: &mut Context) -> Option<String> {
    if org.name.is_none() {
        // US Core Organization (required by Provenance.agent.onBehalfOf) needs a name.
        ctx.report.info(
            "author.organization.no-name",
            "an organization has no name; it was not mapped",
        );
        return None;
    }
    let key = org
        .ids
        .iter()
        .find_map(|id| {
            Some(format!(
                "id:{}|{}",
                id.root.as_deref()?,
                id.extension.as_deref()?
            ))
        })
        .or_else(|| org.name.as_ref().map(|n| format!("name:{n}")))?;
    if let Some(existing) = ctx.organizations.get(&key) {
        return Some(existing.clone());
    }
    let id = ctx.next_id("organization");
    let mut resource = Map::new();
    resource.insert("resourceType".into(), json!("Organization"));
    resource.insert("id".into(), json!(id));
    // C-CDA does not say whether an organization is still active, but US Core requires the
    // element. Assumed true and reported (ADR 0004).
    resource.insert("active".into(), json!(true));
    ctx.report.info(
        "organization.active.assumed",
        "Organization.active is required by US Core but absent in C-CDA; it was set to true",
    );
    let identifiers: Vec<Value> = org
        .ids
        .iter()
        .filter_map(|i| map_identifier(i, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        resource.insert("identifier".into(), json!(identifiers));
    }
    if let Some(name) = &org.name {
        resource.insert("name".into(), json!(name));
    }
    let telecoms: Vec<Value> = org
        .telecoms
        .iter()
        .filter_map(crate::patient::telecom)
        .collect();
    if !telecoms.is_empty() {
        resource.insert("telecom".into(), json!(telecoms));
    }
    let addresses: Vec<Value> = org
        .addresses
        .iter()
        .filter_map(crate::patient::address)
        .collect();
    if !addresses.is_empty() {
        resource.insert("address".into(), json!(addresses));
    }
    let reference = ctx.reference("Organization", &id);
    ctx.organizations.insert(key, reference.clone());
    ctx.push_extra(Value::Object(resource));
    Some(reference)
}

pub(crate) fn register_practitioner(author: &Author, ctx: &mut Context) -> Option<String> {
    let key = person_key(author)?;
    if let Some(existing) = ctx.practitioners.get(&key).cloned() {
        add_role(author, &existing, ctx);
        return Some(existing);
    }
    let id = ctx.next_id("practitioner");
    let mut resource = Map::new();
    resource.insert("resourceType".into(), json!("Practitioner"));
    resource.insert("id".into(), json!(id));
    let identifiers: Vec<Value> = author
        .ids
        .iter()
        .filter_map(|i| map_identifier(i, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        resource.insert("identifier".into(), json!(identifiers));
    }
    if author.family.is_some() || !author.given.is_empty() {
        let mut name = Map::new();
        if let Some(family) = &author.family {
            name.insert("family".into(), json!(family));
        } else {
            // US Core needs name.family (1..1).
            name.insert(
                "_family".into(),
                json!({ "extension": [crate::nullflavor::data_absent_reason_extension("UNK", ctx.report)] }),
            );
        }
        if !author.given.is_empty() {
            name.insert("given".into(), json!(author.given));
        }
        resource.insert("name".into(), json!([name]));
    } else {
        // US Core needs name.family (1..1); with none in the document it is marked absent.
        ctx.report.info(
            "practitioner.name.absent",
            "a practitioner has no name in the document; name.family was marked absent",
        );
        resource.insert(
            "name".into(),
            json!([{ "_family": { "extension": [crate::nullflavor::data_absent_reason_extension("UNK", ctx.report)] } }]),
        );
    }
    let telecoms: Vec<Value> = author
        .telecoms
        .iter()
        .filter_map(crate::patient::telecom)
        .collect();
    if !telecoms.is_empty() {
        resource.insert("telecom".into(), json!(telecoms));
    }
    let addresses: Vec<Value> = author
        .addresses
        .iter()
        .filter_map(crate::patient::address)
        .collect();
    if !addresses.is_empty() {
        resource.insert("address".into(), json!(addresses));
    }
    let reference = ctx.reference("Practitioner", &id);
    ctx.practitioners.insert(key, reference.clone());
    ctx.push_extra(Value::Object(resource));
    add_role(author, &reference, ctx);
    Some(reference)
}

const NUCC: &str = "2.16.840.1.113883.6.101";

/// A `PractitionerRole` for an author's role or specialty code and organization. One per
/// practitioner, role and organization, however many times the author appears.
fn add_role(author: &Author, practitioner: &str, ctx: &mut Context) {
    let role = author
        .role_code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report));
    let organization = author
        .organization
        .as_ref()
        .and_then(|o| register_organization(o, ctx));
    if role.is_none() && organization.is_none() {
        return;
    }
    let key = format!(
        "role:{practitioner}|{}|{}",
        author
            .role_code
            .as_ref()
            .and_then(|c| c.code.as_deref())
            .unwrap_or_default(),
        organization.as_deref().unwrap_or_default()
    );
    if ctx.practitioners.contains_key(&key) {
        return;
    }
    let id = ctx.next_id("practitionerrole");
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("PractitionerRole"));
    r.insert("id".into(), json!(id));
    r.insert("active".into(), json!(true));
    r.insert("practitioner".into(), json!({ "reference": practitioner }));
    if let Some(o) = &organization {
        r.insert("organization".into(), json!({ "reference": o }));
    }
    if let Some(role) = role {
        // A taxonomy code is a specialty; any other role code is the role itself.
        let is_specialty = author
            .role_code
            .as_ref()
            .and_then(|c| c.code_system.as_deref())
            == Some(NUCC);
        if is_specialty {
            r.insert("specialty".into(), json!([role]));
        } else {
            r.insert("code".into(), json!([role]));
        }
    }
    let reference = ctx.reference("PractitionerRole", &id);
    ctx.practitioners.insert(key, reference);
    ctx.push_extra(Value::Object(r));
}

/// A person related to the patient who is named in the header (an informant who is a relative or
/// friend) becomes a `RelatedPerson`.
pub(crate) fn register_related_person(a: &Author, ctx: &mut Context) -> Option<String> {
    if a.family.is_none() && a.given.is_empty() {
        return None;
    }
    let key = format!(
        "related:{}|{}|{}",
        a.given.join(" "),
        a.family.as_deref().unwrap_or_default(),
        a.role_code
            .as_ref()
            .and_then(|c| c.code.as_deref())
            .unwrap_or_default()
    );
    if let Some(existing) = ctx.practitioners.get(&key) {
        return Some(existing.clone());
    }
    let id = ctx.next_id("relatedperson");
    let mut r = Map::new();
    r.insert("resourceType".into(), json!("RelatedPerson"));
    r.insert("id".into(), json!(id));
    r.insert("active".into(), json!(true));
    r.insert("patient".into(), json!({ "reference": ctx.patient }));
    if let Some(rel) = a
        .role_code
        .as_ref()
        .and_then(|c| map_codeable_concept(c, ctx.terminology, ctx.report))
    {
        r.insert("relationship".into(), json!([rel]));
    }
    let mut name = Map::new();
    if let Some(f) = &a.family {
        name.insert("family".into(), json!(f));
    }
    if !a.given.is_empty() {
        name.insert("given".into(), json!(a.given));
    }
    r.insert("name".into(), json!([name]));
    let telecoms: Vec<Value> = a
        .telecoms
        .iter()
        .filter_map(crate::patient::telecom)
        .collect();
    if !telecoms.is_empty() {
        r.insert("telecom".into(), json!(telecoms));
    }
    let addresses: Vec<Value> = a
        .addresses
        .iter()
        .filter_map(crate::patient::address)
        .collect();
    if !addresses.is_empty() {
        r.insert("address".into(), json!(addresses));
    }
    let reference = ctx.reference("RelatedPerson", &id);
    ctx.practitioners.insert(key, reference.clone());
    ctx.push_extra(Value::Object(r));
    Some(reference)
}

/// The resource that stands for an author who may be software or an organization, for places
/// (Composition.author) where those are allowed: a person becomes a Practitioner, software a
/// Device, and an author with only an organization that Organization.
pub(crate) fn register_author_agent(author: &Author, ctx: &mut Context) -> Option<String> {
    if person_key(author).is_some() {
        return register_practitioner(author, ctx);
    }
    if let Some(reference) = register_device(author, ctx) {
        return Some(reference);
    }
    author
        .organization
        .as_ref()
        .and_then(|o| register_organization(o, ctx))
}

/// A `Device` for an author that is software (`assignedAuthoringDevice`). Shared by id and name.
pub(crate) fn register_device(author: &Author, ctx: &mut Context) -> Option<String> {
    let name = author.device_name.as_ref()?;
    let key = format!(
        "device:{}|{}",
        author
            .ids
            .iter()
            .find_map(|id| Some(format!(
                "{}|{}",
                id.root.as_deref()?,
                id.extension.as_deref()?
            )))
            .unwrap_or_default(),
        name
    );
    if let Some(existing) = ctx.practitioners.get(&key) {
        return Some(existing.clone());
    }
    let id = ctx.next_id("device");
    let mut resource = Map::new();
    resource.insert("resourceType".into(), json!("Device"));
    resource.insert("id".into(), json!(id));
    let identifiers: Vec<Value> = author
        .ids
        .iter()
        .filter_map(|i| map_identifier(i, ctx.terminology, ctx.report))
        .collect();
    if !identifiers.is_empty() {
        resource.insert("identifier".into(), json!(identifiers));
    }
    resource.insert(
        "deviceName".into(),
        json!([{ "name": name, "type": "user-friendly-name" }]),
    );
    let reference = ctx.reference("Device", &id);
    ctx.practitioners.insert(key, reference.clone());
    ctx.push_extra(Value::Object(resource));
    Some(reference)
}

/// Where a resource type keeps the author information.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AuthorFields {
    /// The element for the latest author, if the resource has one (`recorder`).
    pub recorder: Option<&'static str>,
    /// The element for the earliest author time (`recordedDate` or `recorded`).
    pub date: &'static str,
}

/// `Condition` and `AllergyIntolerance`.
pub(crate) const RECORDER_FIELDS: AuthorFields = AuthorFields {
    recorder: Some("recorder"),
    date: "recordedDate",
};

/// Add the recorder and recorded date to `resource` and create its `Provenance`.
///
/// `authors` should be the statement's own authors, or its concern's when it has none (the IG
/// recommends that fallback).
pub(crate) fn apply_authors(
    resource: &mut Map<String, Value>,
    fields: AuthorFields,
    authors: &[&Author],
    ctx: &mut Context,
) {
    if authors.is_empty() {
        return;
    }

    // (author, agent reference, organization reference). Software authors are agents of the
    // Provenance (a Device), but a Device cannot be a `recorder`.
    let mut agents: Vec<(&Author, String, Option<String>)> = Vec::new();
    let mut software: Vec<(String, Option<String>)> = Vec::new();
    for author in authors {
        if let Some(practitioner) = register_practitioner(author, ctx) {
            let organization = author
                .organization
                .as_ref()
                .and_then(|org| register_organization(org, ctx));
            agents.push((author, practitioner, organization));
        } else if let Some(device) = register_device(author, ctx) {
            let organization = author
                .organization
                .as_ref()
                .and_then(|org| register_organization(org, ctx));
            software.push((device, organization));
        }
    }

    // Latest author wins `recorder`; later in the document breaks ties.
    // `max_by_key` returns the last maximum, which is what we want for ties.
    if let (Some(field), Some((_, practitioner, _))) = (
        fields.recorder,
        agents
            .iter()
            .max_by_key(|(a, _, _)| a.time.as_deref().map(time_key).unwrap_or_default()),
    ) {
        resource.insert(field.into(), json!({ "reference": practitioner }));
    }

    // Earliest author time wins `recordedDate`; earlier in the document breaks ties.
    let earliest = authors
        .iter()
        .filter_map(|a| a.time.as_deref())
        .min_by_key(|t| time_key(t));
    let recorded = earliest.and_then(|raw| map_datetime(raw, ctx.report));
    if let Some(date) = &recorded {
        resource.insert(fields.date.into(), json!(date));
    }

    if agents.is_empty() && software.is_empty() {
        return;
    }
    // Provenance.recorded is an `instant`: it needs a time and an offset.
    let Some(instant) = recorded.filter(|d| d.contains('T')) else {
        ctx.report.warn(
            "provenance.recorded.unavailable",
            "authors have no time with an offset, so no Provenance was created (recorded is required)",
        );
        return;
    };
    let (Some(resource_type), Some(target_id)) = (
        resource.get("resourceType").and_then(Value::as_str),
        resource.get("id").and_then(Value::as_str),
    ) else {
        return;
    };
    let target = ctx.reference(resource_type, target_id);

    let mut seen: Vec<(String, Option<String>)> = Vec::new();
    let mut agent_json = Vec::new();
    let people = agents.iter().map(|(_, p, o)| (p, o));
    let devices = software.iter().map(|(d, o)| (d, o));
    for (practitioner, organization) in people.chain(devices) {
        let key = (practitioner.clone(), organization.clone());
        if seen.contains(&key) {
            continue;
        }
        let mut agent = Map::new();
        agent.insert(
            "type".into(),
            json!({ "coding": [{ "system": PARTICIPANT_TYPE_SYSTEM, "code": "author" }] }),
        );
        agent.insert("who".into(), json!({ "reference": practitioner }));
        if let Some(org) = organization {
            agent.insert("onBehalfOf".into(), json!({ "reference": org }));
        }
        agent_json.push(Value::Object(agent));
        seen.push(key);
    }
    let id = ctx.next_id("provenance");
    ctx.push_extra(json!({
        "resourceType": "Provenance",
        "id": id,
        "target": [{ "reference": target }],
        "recorded": instant,
        "agent": agent_json,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Report;
    use ccda_model::{AuthorOrganization, Identifier};
    use ccda_terminology::BuiltinTerminology;

    /// An id; only the real-looking ones (10 digits) are NPIs, so short test ids do not trip the
    /// NPI check.
    fn npi(n: &str) -> Identifier {
        Identifier {
            root: Some(if n.len() == 10 {
                "2.16.840.1.113883.4.6".into()
            } else {
                "1.3.6.1.4.1.99999".into()
            }),
            extension: Some(n.into()),
            null_flavor: None,
            ..Identifier::default()
        }
    }

    fn person(n: &str, family: &str, time: Option<&str>) -> Author {
        Author {
            time: time.map(Into::into),
            ids: vec![npi(n)],
            family: Some(family.into()),
            given: vec!["Pat".into()],
            ..Author::default()
        }
    }

    fn target() -> Map<String, Value> {
        let Value::Object(map) = json!({ "resourceType": "Condition", "id": "condition-1" }) else {
            unreachable!()
        };
        map
    }

    fn run(authors: &[Author]) -> (Map<String, Value>, Vec<Value>, Report) {
        let mut report = Report::default();
        let mut ctx = Context::new(&BuiltinTerminology, &mut report, "doc");
        let mut resource = target();
        let refs: Vec<&Author> = authors.iter().collect();
        apply_authors(&mut resource, RECORDER_FIELDS, &refs, &mut ctx);
        let extra = ctx.take_extra();
        (resource, extra, report)
    }

    fn of_type<'a>(extra: &'a [Value], t: &str) -> Vec<&'a Value> {
        extra.iter().filter(|r| r["resourceType"] == t).collect()
    }

    #[test]
    fn recorder_is_latest_and_recorded_date_is_earliest() {
        let authors = [
            person("1111111112", "First", Some("20200101090000-0500")),
            person("2222222228", "Second", Some("20210301090000-0500")),
        ];
        let (resource, extra, report) = run(&authors);
        let practitioners = of_type(&extra, "Practitioner");
        assert_eq!(practitioners.len(), 2);
        assert_eq!(
            resource["recorder"]["reference"],
            fhir_out::urn_for("doc", "Practitioner", "practitioner-2")
        );
        assert_eq!(resource["recordedDate"], "2020-01-01T09:00:00-05:00");

        let provenance = of_type(&extra, "Provenance");
        assert_eq!(provenance.len(), 1);
        assert_eq!(provenance[0]["recorded"], "2020-01-01T09:00:00-05:00");
        assert_eq!(
            provenance[0]["target"][0]["reference"],
            fhir_out::urn_for("doc", "Condition", "condition-1")
        );
        assert_eq!(provenance[0]["agent"].as_array().unwrap().len(), 2);
        assert_eq!(
            provenance[0]["agent"][0]["type"]["coding"][0]["code"],
            "author"
        );
        assert!(report.issues.is_empty());
    }

    #[test]
    fn practitioner_has_identifier_and_name() {
        let (_, extra, _) = run(&[person("1234567893", "Doe", Some("20200101090000-0500"))]);
        assert_eq!(
            of_type(&extra, "Practitioner")[0],
            &json!({
                "resourceType": "Practitioner",
                "id": "practitioner-1",
                "identifier": [{ "system": "http://hl7.org/fhir/sid/us-npi", "value": "1234567893" }],
                "name": [{ "family": "Doe", "given": ["Pat"] }]
            })
        );
    }

    #[test]
    fn the_same_person_becomes_one_practitioner() {
        let authors = [
            person("1111111112", "Doe", Some("20200101090000-0500")),
            person("1111111112", "Doe", Some("20200201090000-0500")),
        ];
        let (_, extra, _) = run(&authors);
        assert_eq!(of_type(&extra, "Practitioner").len(), 1);
        assert_eq!(
            of_type(&extra, "Provenance")[0]["agent"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn organization_is_shared_and_linked_with_on_behalf_of() {
        let mut a = person("1111111112", "A", Some("20200101090000-0500"));
        let mut b = person("2222222228", "B", Some("20200102090000-0500"));
        for author in [&mut a, &mut b] {
            author.organization = Some(AuthorOrganization {
                ids: vec![Identifier {
                    root: Some("1.2.3".into()),
                    extension: Some("ORG".into()),
                    null_flavor: None,
                    ..Identifier::default()
                }],
                name: Some("Example Clinic".into()),
                ..AuthorOrganization::default()
            });
        }
        let (_, extra, _) = run(&[a, b]);
        let orgs = of_type(&extra, "Organization");
        assert_eq!(orgs.len(), 1);
        assert_eq!(orgs[0]["name"], "Example Clinic");
        assert_eq!(orgs[0]["active"], true);
        let agents = &of_type(&extra, "Provenance")[0]["agent"];
        assert_eq!(
            agents[0]["onBehalfOf"]["reference"],
            fhir_out::urn_for("doc", "Organization", "organization-1")
        );
    }

    #[test]
    fn date_only_times_set_recorded_date_but_cannot_make_provenance() {
        let (resource, extra, report) = run(&[person("1111111112", "Doe", Some("20200101"))]);
        assert_eq!(resource["recordedDate"], "2020-01-01");
        assert!(of_type(&extra, "Provenance").is_empty());
        assert_eq!(report.issues[0].code, "provenance.recorded.unavailable");
    }

    #[test]
    fn author_without_time_still_gets_a_recorder() {
        let (resource, extra, report) = run(&[person("1111111112", "Doe", None)]);
        assert!(resource.contains_key("recorder"));
        assert!(!resource.contains_key("recordedDate"));
        assert!(of_type(&extra, "Provenance").is_empty());
        assert_eq!(report.issues[0].code, "provenance.recorded.unavailable");
    }

    #[test]
    fn software_author_is_a_provenance_agent_but_never_the_recorder() {
        let device = Author {
            time: Some("20200101090000-0500".into()),
            device_name: Some("EHR".into()),
            ..Author::default()
        };
        let (resource, extra, report) = run(&[device]);
        assert!(!resource.contains_key("recorder"));
        assert_eq!(of_type(&extra, "Device").len(), 1);
        let provenance = &of_type(&extra, "Provenance")[0];
        assert!(provenance["agent"][0]["who"]["reference"]
            .as_str()
            .unwrap()
            .starts_with("urn:uuid:"));
        assert!(report.issues.is_empty());
    }

    #[test]
    fn a_person_and_software_share_one_provenance_and_the_person_is_the_recorder() {
        let device = Author {
            time: Some("20200101090000-0500".into()),
            device_name: Some("EHR".into()),
            ..Author::default()
        };
        let (resource, extra, _) = run(&[
            device,
            person("1111111112", "Doe", Some("20200101090000-0500")),
        ]);
        assert!(resource.contains_key("recorder"));
        assert_eq!(
            of_type(&extra, "Provenance")[0]["agent"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn organization_without_a_name_is_skipped_with_a_note() {
        let mut author = person("1111111112", "A", Some("20200101090000-0500"));
        author.organization = Some(AuthorOrganization {
            ids: vec![npi("9")],
            name: None,
            ..AuthorOrganization::default()
        });
        let (_, extra, report) = run(&[author]);
        assert!(of_type(&extra, "Organization").is_empty());
        assert!(of_type(&extra, "Provenance")[0]["agent"][0]
            .get("onBehalfOf")
            .is_none());
        assert_eq!(report.issues[0].code, "author.organization.no-name");
    }

    #[test]
    fn people_without_ids_are_keyed_by_name() {
        let nameless_id = |family: &str| Author {
            family: Some(family.into()),
            time: Some("20200101090000-0500".into()),
            ..Author::default()
        };
        let (_, extra, _) = run(&[nameless_id("Doe"), nameless_id("Doe"), nameless_id("Roe")]);
        assert_eq!(of_type(&extra, "Practitioner").len(), 2);
    }

    #[test]
    fn software_with_an_id_is_a_device_not_a_practitioner() {
        let device = Author {
            time: Some("20200101090000-0500".into()),
            ids: vec![npi("2222222228")],
            device_name: Some("EHR".into()),
            ..Author::default()
        };
        let (resource, extra, _) = run(&[device]);
        assert!(!resource.contains_key("recorder"));
        assert!(of_type(&extra, "Practitioner").is_empty());
        assert_eq!(of_type(&extra, "Device").len(), 1);
    }
}
