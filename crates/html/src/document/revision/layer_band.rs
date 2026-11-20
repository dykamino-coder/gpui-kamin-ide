//! Whether the scene layer can record the band of a restyled inline box
//! again in its pieces (`gpui::Window::rerecord_layer_band`).
//!
//! The pieces of an inline box are where its runs meet its lines; a restyle
//! that leaves layout alone keeps them, so the band takes its new colours and
//! corners there. Two changes move pieces even so, and refuse: a band that
//! appears or goes on a box nested in another inline box (the descendant's
//! runs pass from one box's band to the other's), and a band that appears or
//! goes with padding on a box without a border (the band's padding is the
//! band's alone: the line's stacking layer grows or shrinks with it).

use super::patch::{Path, element_at};
use crate::dom::Element;
use crate::paint::box_tree::styled::Refusal;
use crate::style::values::value::Len;

/// What the restyle of inline box `a` to `b` at `path` in `old` asks of its
/// band's pieces.
#[derive(Debug, PartialEq)]
pub(super) enum BandChange {
    /// The box has no piece in any line and takes none: nothing paints.
    Nothing,
    /// Its pieces are painted again from the new style.
    Pieces,
    /// The layer cannot, for the reason named (`paint_path::not_own`).
    Refused(Refusal),
}

fn zero(l: Option<Len>) -> bool {
    match l {
        None => true,
        Some(Len::Px(v)) => v == 0.0,
        Some(_) => false,
    }
}

/// Whether `e` sets padding on any side.
fn padded(e: &Element) -> bool {
    let p = &e.style.used().reset_box_layout.padding;
    ![p.top, p.right, p.bottom, p.left].into_iter().all(zero)
}

/// Whether `e` sets a border width on any side: its band is there with or
/// without a background (`box_paint`).
fn bordered(e: &Element) -> bool {
    let w = e.style.used().borders();
    ![w.top, w.right, w.bottom, w.left].into_iter().all(zero)
}

/// Whether the background of `e` paints a colour.
fn has_fill(e: &Element) -> bool {
    super::layer_write::painted_background(e).is_none_or(|c| c.is_some())
}

/// The change of inline box `a` to `b` (at `path` in the old tree `old`).
pub(super) fn band_change(
    old: &[crate::dom::Node],
    path: &Path,
    a: &Element,
    b: &Element,
) -> BandChange {
    if super::layer_write::painted_background(b).is_none() {
        // A gradient or an image: no colour of a band.
        return BandChange::Refused(Refusal::BandPaint);
    }
    let appears = has_fill(a) != has_fill(b);
    if a.children.is_empty() && !padded(b) && !bordered(b) && !bordered(a) {
        // No text, no spacer: the box has no fragment to paint into.
        return BandChange::Nothing;
    }
    if !appears {
        return BandChange::Pieces;
    }
    let parent = path
        .len()
        .checked_sub(1)
        .and_then(|n| element_at(old, &path[..n]));
    if parent.is_some_and(|p| p.inline && p.style.used().reset_box_layout.display.is_none()) {
        return BandChange::Refused(Refusal::BandNested);
    }
    if padded(b) && !bordered(b) {
        return BandChange::Refused(Refusal::BandGeometry);
    }
    BandChange::Pieces
}
