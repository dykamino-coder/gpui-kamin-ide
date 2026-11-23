//! Reserve locally matching boxes before cross-parent relocation.
//!
//! A source may have several layout fragments. A new fragment must not borrow
//! a box that a later sibling will retain, including an unvisited shared subtree.
//! This pass follows the same changed input paths and stops at shared children;
//! it never enumerates the arena or descends into a retained subtree.

use super::{
    BoxTree,
    claims::Claims,
    level::{Level, Prev, paired},
    publish::Keys,
    source::Source,
};
use crate::dom::{BoxId, Node};

pub(super) fn reserve(
    tree: &BoxTree,
    claims: &mut Claims,
    nodes: &[Node],
    prev: Prev<'_>,
    keys: &Keys<'_>,
    root: BoxId,
    first: BoxId,
) {
    if matches!(prev, Prev::Fresh) {
        return;
    }
    // Local matching reads old links without claiming or rewriting any slot.
    let mut local = Claims::default();
    let mut pending = vec![Level::new(nodes, root, None, prev, true, first)];
    while let Some(level) = pending.last_mut() {
        let Some(node) = level.next_node() else {
            pending.pop();
            continue;
        };
        let Node::Element(e) = node else { continue };
        let source = Source::of(
            e,
            level.parent,
            tree.extent(e, keys),
            (keys.element)(e.node_id),
        );
        let Some(id) = level.take(tree, &mut local, &source) else {
            continue;
        };
        claims.reserve(id);
        let old = match level.prev {
            Prev::Old(old) => paired(old, level.nodes.len(), level.at - 1, e.node_id),
            _ => None,
        };
        if old.is_some_and(|o| o.children.identity() == e.children.identity()) {
            claims.keep(id);
            continue;
        }
        let first = tree.get(id).map_or(BoxId::NONE, |r| r.first);
        pending.push(Level::new(
            &e.children,
            id,
            Some(e),
            old.map_or(Prev::Fresh, |o| Prev::Old(&o.children)),
            true,
            first,
        ));
    }
}
