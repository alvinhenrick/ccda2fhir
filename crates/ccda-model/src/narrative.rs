//! Section narrative (`text`) as FHIR XHTML.
//!
//! The CDA narrative block (StrucDoc) is a small HTML-like language. FHIR allows a restricted
//! XHTML subset in `Narrative.div`. This module copies the elements the two have in common and
//! nothing else: attributes are dropped except `colspan`, `rowspan` and a safe `href`; text is
//! escaped; elements with no FHIR counterpart (`renderMultiMedia`, scripts, unknown tags) keep
//! only their text. The result therefore cannot carry script, style, event handlers or images.

use roxmltree::Node;

/// Deepest element nesting rendered as markup; anything deeper is reduced to its text.
const MAX_DEPTH: usize = 24;

/// The section `text` element as an XHTML `div`, or `None` if it holds no text.
pub(crate) fn xhtml(text: &Node) -> Option<String> {
    if !has_text(text) {
        return None;
    }
    let mut body = String::new();
    render_children(text, &mut body, 0);
    Some(format!(
        "<div xmlns=\"http://www.w3.org/1999/xhtml\">{body}</div>"
    ))
}

fn has_text(node: &Node) -> bool {
    node.descendants()
        .any(|n| n.is_text() && n.text().is_some_and(|t| !t.trim().is_empty()))
}

fn escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // XML 1.0 does not allow these, and the parser already rejects them; be safe anyway.
            c if (c as u32) < 0x20 && !matches!(c, '\n' | '\t' | '\r') => {}
            c => out.push(c),
        }
    }
}

fn render_children(node: &Node, out: &mut String, depth: usize) {
    for child in node.children() {
        if child.is_text() {
            if let Some(t) = child.text() {
                let structural = matches!(
                    node.tag_name().name(),
                    "text" | "list" | "table" | "thead" | "tbody" | "tfoot" | "tr"
                );
                if structural && t.trim().is_empty() {
                    continue;
                }
                // Source indentation is not content: collapse whitespace runs to one space.
                let mut collapsed = String::with_capacity(t.len());
                let mut space = false;
                for c in t.chars() {
                    if c.is_whitespace() {
                        space = true;
                    } else {
                        if space {
                            collapsed.push(' ');
                            space = false;
                        }
                        collapsed.push(c);
                    }
                }
                if space {
                    collapsed.push(' ');
                }
                escape(&collapsed, out);
            }
        } else if child.is_element() {
            render_element(&child, out, depth + 1);
        }
    }
}

fn digits(value: Option<&str>) -> Option<&str> {
    value.filter(|v| !v.is_empty() && v.len() <= 4 && v.bytes().all(|b| b.is_ascii_digit()))
}

fn safe_href(value: &str) -> bool {
    let v = value.trim().to_ascii_lowercase();
    v.starts_with("http://") || v.starts_with("https://") || v.starts_with("mailto:")
}

fn render_element(node: &Node, out: &mut String, depth: usize) {
    let name = node.tag_name().name();
    if depth > MAX_DEPTH {
        // Too deep to trust as markup: keep the words only.
        for t in node.descendants().filter(Node::is_text) {
            if let Some(t) = t.text() {
                escape(t, out);
            }
        }
        return;
    }
    let simple = match name {
        "paragraph" => Some("p"),
        "content" => Some("span"),
        "item" => Some("li"),
        "caption" => Some("caption"),
        "table" => Some("table"),
        "thead" => Some("thead"),
        "tbody" => Some("tbody"),
        "tfoot" => Some("tfoot"),
        "tr" => Some("tr"),
        "sup" => Some("sup"),
        "sub" => Some("sub"),
        "col" | "colgroup" => None,
        _ => None,
    };
    if let Some(tag) = simple {
        out.push('<');
        out.push_str(tag);
        out.push('>');
        render_children(node, out, depth);
        out.push_str("</");
        out.push_str(tag);
        out.push('>');
        return;
    }
    match name {
        "br" => out.push_str("<br/>"),
        "list" => {
            let tag = if node.attribute("listType") == Some("ordered") {
                "ol"
            } else {
                "ul"
            };
            out.push_str(&format!("<{tag}>"));
            render_children(node, out, depth);
            out.push_str(&format!("</{tag}>"));
        }
        "th" | "td" => {
            out.push('<');
            out.push_str(name);
            for attr in ["colspan", "rowspan"] {
                if let Some(v) = digits(node.attribute(attr)) {
                    out.push_str(&format!(" {attr}=\"{v}\""));
                }
            }
            out.push('>');
            render_children(node, out, depth);
            out.push_str("</");
            out.push_str(name);
            out.push('>');
        }
        "linkHtml" => match node.attribute("href").filter(|h| safe_href(h)) {
            Some(href) => {
                out.push_str("<a href=\"");
                escape(href.trim(), out);
                out.push_str("\">");
                render_children(node, out, depth);
                out.push_str("</a>");
            }
            None => render_children(node, out, depth),
        },
        // Images and other media need binary content; the words around them are kept.
        "renderMultiMedia" => {}
        // Anything else (footnotes, unknown tags): keep the text, drop the markup.
        _ => render_children(node, out, depth),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(inner: &str) -> Option<String> {
        let xml = format!(r#"<section xmlns="urn:hl7-org:v3"><text>{inner}</text></section>"#);
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let text = doc.root_element().first_element_child().unwrap();
        xhtml(&text)
    }

    #[test]
    fn tables_lists_and_paragraphs_are_kept() {
        let out = render(
            r#"<paragraph>Hello <content styleCode="Bold">there</content></paragraph>
<list listType="ordered"><item>one</item><item>two</item></list>
<table><thead><tr><th>A</th></tr></thead><tbody><tr><td colspan="2">x</td></tr></tbody></table>"#,
        )
        .unwrap();
        assert!(out.starts_with("<div xmlns=\"http://www.w3.org/1999/xhtml\">"));
        assert!(out.contains("<p>Hello <span>there</span></p>"));
        assert!(out.contains("<ol><li>one</li><li>two</li></ol>"));
        assert!(out.contains("<td colspan=\"2\">x</td>"));
        assert!(!out.contains("styleCode"));
    }

    #[test]
    fn markup_is_escaped_and_unsafe_parts_dropped() {
        let out = render(
            r#"<paragraph>a &lt;script&gt; &amp; b</paragraph>
<linkHtml href="javascript:alert(1)">bad</linkHtml>
<linkHtml href="https://example.org/x">good</linkHtml>
<renderMultiMedia referencedObject="IMG"/>"#,
        )
        .unwrap();
        assert!(out.contains("a &lt;script&gt; &amp; b"));
        assert!(!out.contains("javascript"));
        assert!(out.contains("<a href=\"https://example.org/x\">good</a>"));
        assert!(!out.contains("IMG"));
    }

    #[test]
    fn empty_narrative_is_none() {
        assert_eq!(render("  "), None);
        assert_eq!(render("<paragraph/>"), None);
    }

    #[test]
    fn deep_nesting_is_reduced_to_text() {
        let deep = "<content>".repeat(60) + "words" + &"</content>".repeat(60);
        let out = render(&deep).unwrap();
        assert!(out.contains("words"));
        assert!(out.matches("<span>").count() <= MAX_DEPTH);
    }
}
