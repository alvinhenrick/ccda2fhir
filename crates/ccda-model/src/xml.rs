//! Small helpers over `roxmltree` for reading HL7 v3 elements.

use roxmltree::Node;

/// The HL7 v3 namespace that C-CDA elements live in.
pub(crate) const HL7_NS: &str = "urn:hl7-org:v3";
/// The XML Schema instance namespace (for `xsi:type`).
pub(crate) const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// True if `node` is an element named `name` in the HL7 v3 namespace.
pub(crate) fn is_hl7(node: &Node, name: &str) -> bool {
    node.is_element()
        && node.tag_name().name() == name
        && node.tag_name().namespace() == Some(HL7_NS)
}

/// First direct child element with the given HL7 name.
pub(crate) fn child<'a, 'input>(node: &Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|c| is_hl7(c, name))
}

/// All direct child elements with the given HL7 name.
pub(crate) fn children<'a, 'input>(
    node: &Node<'a, 'input>,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> + 'a {
    node.children().filter(move |c| is_hl7(c, name))
}

/// Follow a path of child element names, e.g. `["recordTarget", "patientRole"]`.
pub(crate) fn path<'a, 'input>(
    node: &Node<'a, 'input>,
    names: &[&str],
) -> Option<Node<'a, 'input>> {
    names
        .iter()
        .try_fold(*node, |current, name| child(&current, name))
}

/// A non-empty attribute value.
pub(crate) fn attr(node: &Node, name: &str) -> Option<String> {
    node.attribute(name)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

/// Text content of an element with whitespace runs collapsed; `None` if empty.
pub(crate) fn text_of(node: &Node) -> Option<String> {
    let raw: String = node
        .descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect();
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    (!collapsed.is_empty()).then_some(collapsed)
}
