//! The flow blocks that hold given elements.

use crate::dom::{Element, Node};

/// The flow blocks that hold the elements `ids`; `None` if one is in none
/// (a root box, or an element out of the flow).
pub(super) fn holders(flow: &[Node], ids: &[u64]) -> Option<std::collections::HashSet<u64>> {
    ids.iter()
        .map(|&id| block(&flow[holder(flow, id)?]).map(|e| e.node_id))
        .collect()
}

/// Позиция блока, содержащего ключ: числового порядка у NodeKey нет.
pub(super) fn holder(flow: &[Node], id: u64) -> Option<usize> {
    flow.iter()
        .position(|n| block(n).is_some_and(|e| holds(e, id)))
}

/// The element `n`, if it is one.
fn block(n: &Node) -> Option<&Element> {
    match n {
        Node::Element(e) => Some(e),
        Node::Text(_) => None,
    }
}

/// The subtree of `e` has the element `id`.
fn holds(e: &Element, id: u64) -> bool {
    let mut pending = vec![e];
    while let Some(e) = pending.pop() {
        if e.node_id == id {
            return true;
        }
        pending.extend(e.children.iter().filter_map(|n| match n {
            Node::Element(c) => Some(c),
            Node::Text(_) => None,
        }));
    }
    false
}
