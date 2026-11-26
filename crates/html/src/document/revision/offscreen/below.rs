//! The area of an inline box the layer's frame never placed: no fragment of
//! it, nor of a band owner holding it, describes the layer (a paragraph
//! no native paragraph painted).
//!
//! Such a box lies in the ink overflow its paragraph's box kept from the
//! layer's frame (`layout::retained::measured`): every primitive the
//! paragraph offered, culled and deferred paint included, and the
//! paragraph's own box, which holds its lines. Gecko bounds the paint of a
//! frame the same way, by the ink overflow it keeps with the frame's border
//! box. The lines hold each inline box on them but for the box's padding and
//! border, which a restyle that keeps layout keeps: the area is grown by
//! them and by the reach of the box's new ink. A box whose other paragraphs
//! placed fragments in the frame holds lines its ink does not: unknown.
//!
//! The measure is of the old paint. The new paint of the box lies where its
//! old box did, unless an element on the way moves it (positioned or
//! transformed): then the area is unknown.

use crate::document::boxes::BoxTree;
use crate::dom::Element;
use crate::layout::retained::LayoutSlot;
use crate::layout::reuse::Store;
use crate::style::computed::Position;
use crate::style::values::value::Len;
use gpui::{Bounds, ScaledPixels};

/// The ink overflow of the nearest ancestor in `up` (outermost first) of
/// box `b` that kept one from the layer's frame, grown by `b`'s padding,
/// border and `outset`, device pixels at the layer's scale. `None` when no
/// ancestor kept one or it is unknown, an element between it and `b` moves
/// `b`, or `b`'s padding or border is no length.
pub(super) fn below(
    store: &Store,
    boxes: &BoxTree,
    up: &[&Element],
    b: &Element,
    outset: f32,
) -> Option<Bounds<ScaledPixels>> {
    let slot = |e: &Element| boxes.slot(boxes.box_of(e.node_id)?);
    let nearest = up
        .iter()
        .rposition(|e| slot(e).is_some_and(|s| unplaced(store, s)))?;
    let kept = slot(up[nearest])?;
    // The ink is of the paragraphs that placed no fragments: a box with
    // fragments of the frame too has lines it does not hold.
    if kept
        .inline
        .as_ref()
        .is_some_and(|f| store.fragments_hold(f.stamp()))
    {
        return None;
    }
    if moves(b) || up[nearest + 1..].iter().any(|e| moves(e)) {
        return None;
    }
    let reach = (outset + edges(b)?) * store.shown_scale();
    let (_, ink) = kept.ink?;
    Some(ink?.dilate(ScaledPixels(reach)))
}

/// Whether paragraphs of the box of `slot` placed no fragments in the
/// layer's frame (`layout::retained::measured`).
pub(super) fn unplaced(store: &Store, slot: &LayoutSlot) -> bool {
    slot.ink
        .is_some_and(|(stamp, _)| store.fragments_hold(stamp))
}

/// Whether `e` places its box away from the flow its ancestor laid out.
fn moves(e: &Element) -> bool {
    let style = e.style.used();
    !matches!(
        style.reset_positioned.position,
        None | Some(Position::Static)
    ) || style.reset_transforms.transform.is_some()
        || style.reset_transforms.translate.is_some()
}

/// The widest padding plus border of `e` on any side, window pixels;
/// `None` for a padding or border no length gives (a percentage).
fn edges(e: &Element) -> Option<f32> {
    let px = |l: Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(v.abs()),
        Some(_) => None,
    };
    let style = e.style.used();
    let (p, w) = (&style.reset_box_layout.padding, style.borders());
    let mut widest: f32 = 0.0;
    for (pad, border) in [
        (p.top, w.top),
        (p.right, w.right),
        (p.bottom, w.bottom),
        (p.left, w.left),
    ] {
        widest = widest.max(px(pad)? + px(border)?);
    }
    Some(widest)
}
