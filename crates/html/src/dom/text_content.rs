//! Текст поддерева DOM и разрезание узлов по смещению в нём.

use crate::dom::Node;

mod split;
pub(crate) use split::split_nodes;

pub(crate) fn gather_text(nodes: &[Node], out: &mut String) {
    let mut pending = vec![nodes.iter()];
    while let Some(nodes) = pending.last_mut() {
        match nodes.next() {
            Some(Node::Text(t)) => out.push_str(t),
            Some(Node::Element(e)) => pending.push(e.children.iter()),
            None => {
                pending.pop();
            }
        }
    }
}
