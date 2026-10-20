//! Текст поддерева DOM и разрезание узлов по смещению в нём.

use crate::dom::Node;

mod split;
pub(crate) use split::split_nodes;

pub(crate) fn gather_text(nodes: &[Node], out: &mut String) {
    for n in nodes {
        match n {
            Node::Text(t) => out.push_str(t),
            Node::Element(e) => gather_text(&e.children, out),
        }
    }
}
