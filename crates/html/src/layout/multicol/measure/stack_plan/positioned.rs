//! Positioned source order before the normal children are filtered or grouped.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::FlowMargins;
use crate::style::computed::Position;

pub(super) fn statics(
    host: &Element,
    normal: Option<&[(Element, FlowMargins)]>,
) -> Vec<(usize, Element)> {
    let mut at = 0;
    let mut out = Vec::new();
    for node in host.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(child) = node else {
            at += 1;
            continue;
        };
        if matches!(
            child.style.used().reset_positioned.position,
            Some(Position::Absolute | Position::Fixed)
        ) {
            // A shared float context retains its placeholders inside its own
            // normal copy. Its recorded static rectangle supplies that anchor.
            if !normal.is_some_and(|kids| kids.iter().any(|(c, _)| contains(c, child.node_id))) {
                out.push((at, child.clone()));
            }
        } else if !out_of_flow(&child.style.used()) {
            at += 1;
        }
    }
    out
}

fn contains(host: &Element, id: u64) -> bool {
    let mut pending = vec![host];
    while let Some(node) = pending.pop() {
        if node.node_id == id {
            return true;
        }
        pending.extend(node.children.iter().filter_map(|n| match n {
            Node::Element(c) => Some(c),
            _ => None,
        }));
    }
    false
}
