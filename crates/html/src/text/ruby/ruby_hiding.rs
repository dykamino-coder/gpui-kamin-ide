//! Ruby annotation hiding after pairing, CSS Ruby 1 §hiding.
//! Compare textContent before whitespace collapsing and text transformation.

use crate::style::computed::Computed;
use crate::dom::Node;

pub(super) fn text(nodes: &[Node]) -> String {
    let mut out = String::new();
    for node in nodes {
        match node {
            Node::Text(t) => out.push_str(t),
            Node::Element(e) => out.push_str(&text(&e.children)),
        }
    }
    out
}

pub(super) fn hidden(annotation: &[Node], base: &str, container: &Computed) -> bool {
    // The renderer uses separate pairing for auto; explicit merge disables
    // hiding. Visibility on the annotation inherits from its container.
    if container.ruby_merge == Some(1) {
        return false;
    }
    let collapsed = if let [Node::Element(e)] = annotation {
        e.style.collapsed.or(container.collapsed)
    } else {
        container.collapsed
    };
    collapsed == Some(true) || text(annotation) == base
}
