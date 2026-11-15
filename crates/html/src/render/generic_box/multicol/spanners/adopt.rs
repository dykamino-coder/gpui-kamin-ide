//! Может ли prepaint рисовать меру спаннер-потока как есть (`Adopt`).

use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;

/// Whether a measure build of the flow of `e` can be painted as it is
/// (`Adopt`): no fragmentation recorder, no gap collector of an outer
/// container, and no box inside that a containing block or the scroll port
/// places (`absolute`, `fixed`, `sticky`), since the measure positions
/// with a fresh collector.
pub(super) fn adoptable(e: &Element, opts: &RenderOpts) -> bool {
    use crate::style::computed::Position;
    if opts.frag.watch.is_some() || opts.cx.deferred.gap_items.is_some() {
        return false;
    }
    let mut pending = vec![e];
    while let Some(e) = pending.pop() {
        if matches!(
            e.style.used().reset_positioned.position,
            Some(Position::Absolute | Position::Fixed | Position::Sticky)
        ) {
            return false;
        }
        pending.extend(e.children.iter().filter_map(|n| match n {
            Node::Element(k) => Some(k),
            _ => None,
        }));
    }
    true
}
