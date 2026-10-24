//! Язык документа и режим совместимости: данные HTML не включают XHTML,
//! HTML-адаптер и настоящий XML проверяются отдельными входами.

use super::*;

const XHTML: &str = "http://www.w3.org/1999/xhtml";
const ATTR_CSS: &str = "p::before { content: attr(DATA-X, 'fallback') }";

fn pseudo_text(nodes: &[Node]) -> Vec<String> {
    let mut out = Vec::new();
    for node in nodes {
        if let Node::Element(element) = node {
            if element.tag == "::before" {
                out.push(
                    element
                        .children
                        .iter()
                        .filter_map(|child| match child {
                            Node::Text(text) => Some(text.as_str()),
                            _ => None,
                        })
                        .collect(),
                );
            }
            out.extend(pseudo_text(&element.children));
        }
    }
    out
}

#[test]
fn html_data_cannot_change_quirks_or_attribute_case_rules() {
    for data in [
        format!("<div>{XHTML}</div>"),
        format!("<!--{XHTML}-->"),
        format!("<script>'{XHTML}'</script>"),
        format!("<style>/*{XHTML}*/</style>"),
        format!("<div data-uri='{XHTML}'></div>"),
        format!("<svg xmlns='{XHTML}'></svg>"),
        "<?xml version='1.0'?>".into(),
    ] {
        let html = format!("{data}<p data-x='pass'></p>");
        let parsed = parse(&html, ATTR_CSS);
        assert!(parsed.doc.quirks(), "{data}");
        assert_eq!(pseudo_text(&parsed), ["pass"], "{data}");
        let standards = parse(&format!("<!doctype html>{html}"), ATTR_CSS);
        assert!(!standards.doc.quirks(), "{data}");
        assert_eq!(pseudo_text(&standards), ["pass"], "{data}");
    }
}

#[test]
fn html_xhtml_adapter_uses_parsed_root_beyond_the_old_prefix_window() {
    let html = format!(
        "<!--{}--><html xmlns='{XHTML}'><body><p data-x='pass'/><p/></body></html>",
        "prolog ".repeat(400),
    );
    let parsed = parse(&html, ATTR_CSS);
    assert!(!parsed.doc.quirks());
    // Самозакрытые p — соседи: текст каждого ::before присутствует.
    assert_eq!(pseudo_text(&parsed), ["fallback", "fallback"]);
    let html = first_element(&parsed);
    assert_eq!(html.tag, "p");
    assert!(
        html.children
            .iter()
            .all(|node| { !matches!(node, Node::Element(element) if element.tag == "p") })
    );
}

#[test]
fn html_xhtml_adapter_accepts_a_parsed_namespace_character_reference() {
    let parsed = parse(
        "<html xmlns='http://www.w3.org/1999/xht&#109;l'><p data-x='pass'/></html>",
        ATTR_CSS,
    );
    assert!(!parsed.doc.quirks());
    assert_eq!(pseudo_text(&parsed), ["fallback"]);
}

#[test]
fn real_xml_gets_case_rules_from_the_host_language() {
    for namespace in ["", " xmlns='http://www.w3.org/1999/xht&#109;l'"] {
        let xml = format!("<root{namespace}><p data-x='lower'/><p DATA-X='upper'/></root>");
        let parsed = parse_xml(&xml, ATTR_CSS);
        assert!(!parsed.doc.quirks());
        assert_eq!(pseudo_text(&parsed), ["fallback", "upper"]);
    }
}

#[test]
fn xml_shadow_attributes_do_not_acquire_html_normalization() {
    let xml =
        format!("<html xmlns='{XHTML}'><template SHADOWROOTMODE='open'><p/></template></html>");
    let dom = super::super::xml::xml_dom(&xml).unwrap();
    assert!(!super::super::shadow::has_declarative_shadow(&dom.document));
    let parsed = parse_xml(&xml, "");
    assert!(!parsed.doc.quirks());
}

#[test]
fn typed_attr_uses_host_language_case_rules_in_the_style_walk() {
    use crate::style::values::value::Len;
    let css = "p { width: attr(DATA-X px, 7px) }";
    for (source, xml, expected) in [
        ("<p data-x='12'/>", false, 12.0),
        ("<p data-x='12'/>", true, 7.0),
        ("<p DATA-X='12'/>", true, 12.0),
    ] {
        let parsed = if xml {
            parse_xml(source, css)
        } else {
            parse(source, css)
        };
        assert_eq!(
            first_element(&parsed).style.cascaded().width,
            Some(Len::Px(expected))
        );
    }
}

#[test]
fn raw_string_attributes_use_each_pseudo_elements_originating_element() {
    for kind in ["", "raw-string", "RAW-STRING"] {
        let parsed = parse(
            "<!doctype html><div data-x='x'></div><div data-x='y'></div>",
            &format!("div::before {{ content: attr(data-x {kind}) }}"),
        );
        assert_eq!(pseudo_text(&parsed), ["x", "y"], "{kind}");
    }
}
