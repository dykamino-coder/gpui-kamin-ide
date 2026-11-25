//! The area of an inline box the layer's frame never placed: no fragment of
//! it, nor of a band owner holding it, describes the layer (a paragraph the
//! frame did not paint, or painted as pieces of a fragmented flow).
//!
//! Such a box is bounded by its nearest ancestor with a run in the layer:
//! normal flow lays the ancestor's contents out from its placed top onward
//! (lines follow its content edge, overflow runs to the block and inline
//! ends), so none of the box's ink lies above that top, less the reach of its
//! own ink. Columns move their contents to the top of their container's
//! column boxes (css-multicol-1 §3): under a multi-column container the top
//! is its own. The area is everything below that top; it never misses a
//! frame that shows anything from there down, however far a scroll jumps.
//! An element between the ancestor and the box that moves the box itself (a
//! positioned or transformed one) bounds nothing, and the area is unknown.

use crate::dom::Element;
use crate::style::computed::Position;
use gpui::{Bounds, LayerHandle, ScaledPixels, point, size};

/// How far, window pixels, the ink of a line reaches above its band: glyph
/// overshoot under a small line height, emphasis marks, an overline
/// (the reach the fragments of a paragraph painted whole allow, `cull`).
const REACH: f32 = 64.;

/// The half-plane below the top of the nearest ancestor in `up` (outermost
/// first) with a run in `layer`, or of the outermost multi-column container
/// holding it, grown upward by `outset` and `REACH`, device pixels at
/// `scale`. `None` when no ancestor has a run, the container has none, or an
/// element between the ancestor and the box `b` moves the box.
pub(super) fn below(
    layer: &LayerHandle,
    up: &[&Element],
    b: &Element,
    outset: f32,
    scale: f32,
) -> Option<Bounds<ScaledPixels>> {
    let area = |e: &Element| {
        u32::try_from(e.node_id)
            .ok()
            .and_then(|o| layer.box_area(o))
    };
    let nearest = up.iter().rposition(|e| area(e).is_some())?;
    if moves(b) || up[nearest + 1..].iter().any(|e| moves(e)) {
        return None;
    }
    let anchor = up[..=nearest]
        .iter()
        .position(|e| columns(e))
        .unwrap_or(nearest);
    let top = area(up[anchor])?.origin.y.0 - (outset + REACH) * scale;
    // Far enough to cover any document and still add in `f32`.
    const FAR: f32 = 1.0e9;
    Some(Bounds {
        origin: point(ScaledPixels(-FAR), ScaledPixels(top)),
        size: size(ScaledPixels(2.0 * FAR), ScaledPixels(FAR)),
    })
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

/// Whether `e` is a multi-column container.
fn columns(e: &Element) -> bool {
    let f = &e.style.used().reset_fragmentation;
    f.column_count.is_some() || f.column_width.is_some()
}
