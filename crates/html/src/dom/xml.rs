//! XML documents (`image/svg+xml`): the tree comes from an XML parser, not the HTML one.
//!
//! DOM §"XML documents" / HTML §16.? "Parsing XHTML documents": an XML document is
//! built by an XML parser — no implied `html`/`body`, no foreign-content breakout
//! (`<meta>` inside `<svg>` stays a child of `<svg>`), namespaces and prefixes come
//! from the markup. The tree is handed to the same `RcDom` the HTML parser fills, so
//! the cascade and the walk below are shared (Blink `XMLDocumentParser` likewise
//! builds the ordinary DOM). A not-well-formed document has no tree: Blink/Gecko
//! show a parse-error page; we render an empty document.

use html5ever::interface::{ElementFlags, NodeOrText, TreeSink};
use html5ever::tendril::StrTendril;
use html5ever::{Attribute, LocalName, Namespace, Prefix, QualName};
use markup5ever_rcdom::{Handle, RcDom};
use std::cell::Cell;

thread_local! {
    /// The document being parsed is an XML document (set by `parse_xml`).
    static XML: Cell<bool> = const { Cell::new(false) };
}

/// Whether the document being parsed now is an XML document.
pub(crate) fn is_xml() -> bool {
    XML.with(|x| x.get())
}

/// Parse `markup` as an XML document with the full cascade of `parse`.
pub fn parse_xml(markup: &str, extra_css: &str) -> Vec<super::Node> {
    XML.with(|x| x.set(true));
    let nodes = super::parse(markup, extra_css);
    XML.with(|x| x.set(false));
    nodes
}

/// The same as XML into the fresh state of the current document (`Document`).
pub(crate) fn parse_current_xml(
    markup: &str,
    extra_css: &str,
    media: crate::style::css::Media,
) -> Vec<super::Node> {
    XML.with(|x| x.set(true));
    let nodes = super::parse_current(markup, extra_css, media);
    XML.with(|x| x.set(false));
    nodes
}

/// Build the DOM of an XML document.
pub(crate) fn xml_dom(markup: &str) -> RcDom {
    let dom = RcDom::default();
    let opts = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    if let Ok(doc) = roxmltree::Document::parse_with_options(markup, opts) {
        let root = dom.get_document();
        for child in doc.root().children() {
            append_node(&dom, &root, child);
        }
    }
    dom
}

fn qual(prefix: Option<&str>, ns: Option<&str>, local: &str) -> QualName {
    QualName::new(
        prefix.filter(|p| !p.is_empty()).map(Prefix::from),
        Namespace::from(ns.unwrap_or("")),
        LocalName::from(local),
    )
}

fn append_node(dom: &RcDom, parent: &Handle, node: roxmltree::Node) {
    match node.node_type() {
        roxmltree::NodeType::Element => {
            let tag = node.tag_name();
            let name = qual(
                tag.namespace().and_then(|ns| node.lookup_prefix(ns)),
                tag.namespace(),
                tag.name(),
            );
            let attrs = node
                .attributes()
                .map(|a| Attribute {
                    name: qual(
                        a.namespace().and_then(|ns| node.lookup_prefix(ns)),
                        a.namespace(),
                        a.name(),
                    ),
                    value: StrTendril::from(a.value()),
                })
                .collect();
            let el = dom.create_element(name, attrs, ElementFlags::default());
            for child in node.children() {
                append_node(dom, &el, child);
            }
            dom.append(parent, NodeOrText::AppendNode(el));
        }
        roxmltree::NodeType::Text => {
            if let Some(t) = node.text() {
                dom.append(parent, NodeOrText::AppendText(StrTendril::from(t)));
            }
        }
        roxmltree::NodeType::Comment => {
            let c = dom.create_comment(StrTendril::from(node.text().unwrap_or("")));
            dom.append(parent, NodeOrText::AppendNode(c));
        }
        _ => {}
    }
}
