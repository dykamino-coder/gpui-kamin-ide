//! Держатели потомков хоста на их запланированных местах в порядке CSS 2.1
//! Appendix E. Группа paint держит свой бокс и потомков.

use super::kid::{Kid, Kind};
use super::{Layer, Slot, flatten, frame};
use crate::paint::context::PaintCx;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};
use std::cell::Cell;
use std::rc::Rc;

/// What every holder of one host shares.
pub(super) struct Placing<'a> {
    pub(super) paint: &'a Rc<PaintCx>,
    /// Inline size of the host's containing block.
    pub(super) cb: f32,
    /// Block size of the plan.
    pub(super) height: f32,
    pub(super) vert: Option<bool>,
    pub(super) inline_up: bool,
}

/// Holders for `kids` in paint order: block-level boxes (step 4), floats
/// (step 5), inline content (step 7), then paint groups (step 8), each at
/// its physical place relative to `origin`.
pub(super) fn holders(
    kids: &[Kid],
    slots: &[Slot],
    at: &Placing,
    origin: (f32, f32),
) -> Vec<AnyElement> {
    let mut flat: Vec<(&Kid, &Slot)> = Vec::new();
    for which in [Layer::Block, Layer::Float, Layer::Inline, Layer::Group] {
        flatten(kids, slots, which, &mut flat);
    }
    flat.into_iter()
        .filter(|(kid, _)| !matches!(kid.kind, Kind::Strut(_)))
        .map(|(kid, s)| holder(kid, s, at, origin))
        .collect()
}

fn holder(kid: &Kid, s: &Slot, at: &Placing, origin: (f32, f32)) -> AnyElement {
    let el = frame(
        kid.kind,
        s.avail,
        (kid.build)(at.cb, s.avail, s.shapes.clone(), s.h, None),
        Rc::new(Cell::new(None)),
        at.vert.is_some(),
        at.paint,
    );
    // Логическое место → физическое (Blink
    // `writing_mode_converter.cc:80-110`): line-left is the bottom
    // edge in sideways-lr, so subtract the measured inline extent.
    let (left, top) = match at.vert {
        None => (s.x, s.y),
        Some(true) => (at.height - s.y - s.b, s.x),
        Some(false) => (s.y, if at.inline_up { at.cb - s.x - s.i } else { s.x }),
    };
    let group = kid
        .nest
        .as_ref()
        .and_then(|n| n.group.as_ref().map(|g| (n, g)));
    let el = match group {
        // A nested block is horizontal (`band_nest_block`): the group's
        // container is its border box, and its descendants are placed from
        // that box's corner.
        Some((nest, (group, _))) => {
            let container = div()
                .relative()
                .w(px(s.i))
                .h(px(s.b))
                .child(div().absolute().left(px(0.0)).top(px(0.0)).child(el))
                .children(holders(&nest.kids, &s.kids, at, (left, top)));
            group(container)
        }
        None => el,
    };
    let held = div()
        .absolute()
        .left(px(left - origin.0))
        .top(px(top - origin.1))
        .child(el)
        .into_any_element();
    // A paint group is painted on the level of positioned boxes, after the
    // inline foreground of its stacking context (CSS Color 4 §opacity,
    // CSS Transforms 1 §transform-rendering).
    match group {
        Some((_, (_, key))) => gpui::PaintLast::new(held).key(*key).into_any_element(),
        None => held,
    }
}
