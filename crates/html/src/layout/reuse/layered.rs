//! Positioned boxes and own layers inside a block replayed on screen
//! (`replay`): the block's recorded layout, prepaint and paint hold them only
//! if they stay inside it. Blink reuses the cached subsequence of a paint
//! layer whose subtree needs no repaint (`PaintLayerPainter::Paint`,
//! paint_layer_painter.cc:412-424); a change of transform, opacity or clip
//! does not invalidate the items there, as they refer to paint property
//! nodes. Our replay copies commands with those values baked in, so the
//! subtree compare (`Source::same`) takes any such change as a rebuild.
//! Gecko's retained display list rebuilds what lies under a modified
//! out-of-flow frame up to its containing block
//! (RetainedDisplayListBuilder.cpp:919-955); here the containing block of
//! every out-of-flow box must be inside the block, so its geometry and paint
//! are the block's own:
//! - the block itself is in flow or relatively positioned (its place is
//!   checked by `ReplaySlot::valid`);
//! - an absolutely positioned box has an ancestor inside the block that
//!   establishes its containing block (`box_predicates::establishes_cb`, the
//!   predicate the positioning layers follow);
//! - a fixed box has one that contains fixed boxes (not by `position`);
//! - no sticky box and no anchor positioning, which read outside geometry.
//!
//! The build checks the rest: no box of the block waits in a layer outside it
//! (`Mark::queued`), and the paint holds no deferred draw
//! (`Window::retained_replayable`). Any doubt is a rebuild.

use crate::dom::{Element, Node};
use crate::style::computed::{Computed, Position, box_predicates::establishes_cb};

/// Whether every out-of-flow box of the subtree of `e` has its containing
/// block inside it (see the module).
pub(super) fn contained(e: &Element) -> bool {
    let root = e.style.used();
    if !matches!(
        root.reset_positioned.position,
        None | Some(Position::Static | Position::Relative)
    ) || anchored(&root)
    {
        return false;
    }
    let mut pending: Vec<(&Element, bool, bool)> = children(e)
        .map(|c| (c, establishes_cb(&root), contains_fixed(&root)))
        .collect();
    while let Some((e, abs, fixed)) = pending.pop() {
        let style = e.style.used();
        let placed = match style.reset_positioned.position {
            None | Some(Position::Static | Position::Relative) => true,
            Some(Position::Absolute) => abs,
            Some(Position::Fixed) => fixed,
            Some(Position::Sticky) => false,
        };
        if !placed || anchored(&style) {
            return false;
        }
        let (abs, fixed) = (
            abs || establishes_cb(&style),
            fixed || contains_fixed(&style),
        );
        pending.extend(children(e).map(|c| (c, abs, fixed)));
    }
    true
}

fn children(e: &Element) -> impl Iterator<Item = &Element> {
    e.children.iter().filter_map(|n| match n {
        Node::Element(c) => Some(c),
        Node::Text(_) => None,
    })
}

/// A containing block for fixed boxes: `establishes_cb` but for `position`,
/// which contains only absolutely positioned boxes (CSS 2.1 §10.1 item 3).
fn contains_fixed(c: &Computed) -> bool {
    let mut unpositioned = c.clone();
    unpositioned.reset_positioned.position = None;
    establishes_cb(&unpositioned)
}

/// Anchor positioning reads the geometry of other boxes.
fn anchored(c: &Computed) -> bool {
    let p = &c.reset_positioned;
    p.anchor_name.is_some()
        || p.position_anchor.is_some()
        || p.position_area.is_some()
        || p.implicit_anchor.is_some()
        || !p.position_try_fallbacks.is_empty()
}

#[cfg(test)]
mod tests;
