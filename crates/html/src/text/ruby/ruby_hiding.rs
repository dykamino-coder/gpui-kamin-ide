//! Сокрытие аннотации руби после спаривания (css-ruby-1 §hiding): аннотация прячется, когда её
//! textContent до схлопывания пробелов и преобразования регистра совпадает с базой или она
//! свёрнута `visibility: collapse`; явный `ruby-merge: merge` сокрытие отключает.

use crate::dom::Node;
use crate::style::computed::Computed;

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
        e.style.computed().collapsed.or(container.collapsed)
    } else {
        container.collapsed
    };
    collapsed == Some(true) || text(annotation) == base
}
