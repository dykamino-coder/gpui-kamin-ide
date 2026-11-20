//! The flow blocks that hold given elements.

use super::super::Record;
use crate::dom::{Element, Node};
use std::{collections::HashMap, rc::Rc};

/// The flow blocks that hold the elements `ids`; `None` if one is in none
/// (a root box, or an element out of the flow).
pub(super) fn holders(flow: &[Node], ids: &[u64]) -> Option<std::collections::HashSet<u64>> {
    ids.iter()
        .map(|&id| block(&flow[holder(flow, id)?]).map(|e| e.node_id))
        .collect()
}

/// The position of the block holding a key: NodeKey has no numeric order.
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

/// Pair recorded flow blocks by identity. Identical prefixes require no index.
/// Only displaced blocks with existing records occupy the temporary table.
pub(super) fn recorded_pairs<'a, 'r>(
    old: &'a [Node],
    new: &'a [Node],
    records: &'r HashMap<u64, Rc<Record>>,
) -> impl Iterator<Item = (&'r Record, &'a Element, &'a Element)> {
    let mut previous = old.iter().filter_map(block).peekable();
    let mut remaining = None;
    let new = if records.is_empty() { &[] } else { new };
    new.iter().filter_map(block).filter_map(move |next| {
        if remaining.is_none()
            && previous
                .peek()
                .is_some_and(|old| old.node_id == next.node_id)
        {
            let old = previous.next()?;
            return Some((records.get(&old.node_id)?.as_ref(), old, next));
        }
        let by_id = remaining.get_or_insert_with(|| {
            let mut map = HashMap::new();
            for old in previous.by_ref() {
                if let Some(record) = records.get(&old.node_id) {
                    // A repeated key cannot identify a displaced block.
                    map.entry(old.node_id)
                        .and_modify(|entry| *entry = None)
                        .or_insert(Some((record.as_ref(), old)));
                }
            }
            map
        });
        by_id
            .remove(&next.node_id)
            .flatten()
            .map(|(record, old)| (record, old, next))
    })
}
