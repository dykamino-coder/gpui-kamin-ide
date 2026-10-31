//! Текст поддерева DOM и разрезание узлов по смещению в нём.

use crate::dom::Node;

mod split;
pub use split::split_nodes;

pub fn gather_text(nodes: &[Node], out: &mut String) {
    crate::work::add!(SubtreeQueries, 1);
    let mut pending = vec![nodes.iter()];
    while let Some(nodes) = pending.last_mut() {
        match nodes.next() {
            Some(Node::Text(t)) => {
                crate::work::add!(SubtreeNodesVisited, 1);
                out.push_str(t);
            }
            Some(Node::Element(e)) => {
                crate::work::add!(SubtreeNodesVisited, 1);
                pending.push(e.children.iter());
            }
            None => {
                pending.pop();
            }
        }
    }
}
