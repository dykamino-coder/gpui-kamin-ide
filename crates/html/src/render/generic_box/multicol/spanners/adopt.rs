//! Может ли prepaint рисовать меру спаннер-потока как есть (`Adopt`).

use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;

/// Whether a measure build of the flow of `e` can be painted as it is
/// (`Adopt`): no fragmentation recorder, no gap collector of an outer
/// container, and no box inside that is positioned or makes a stacking
/// context. The measure positions with a fresh collector, and it builds in
/// the layout phase: a step-8 wrapper (CSS 2.1 Appendix E) would take its
/// paint key before the earlier deferred siblings that build in prepaint.
pub(super) fn adoptable(e: &Element, opts: &RenderOpts) -> bool {
    use crate::style::computed::Position;
    if opts.frag.watch.is_some() || opts.cx.deferred.gap_items.is_some() {
        return false;
    }
    let mut pending = vec![e];
    while let Some(e) = pending.pop() {
        let style = e.style.used();
        if !matches!(
            style.reset_positioned.position,
            None | Some(Position::Static)
        ) || crate::style::computed::stacking::stacking_context(&style)
        {
            return false;
        }
        pending.extend(e.children.iter().filter_map(|n| match n {
            Node::Element(k) => Some(k),
            _ => None,
        }));
    }
    true
}
