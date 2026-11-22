//! The DevTools `why` record of a layer write the layer refuses: which box,
//! which painter it was recorded with and why it cannot take the new paint.

use crate::paint::box_tree::styled::Refusal;
use gpui::LayerPainter;

/// `not_own`: box `owner` paints what the layer cannot take (`reason`).
#[track_caller]
pub(super) fn not_own(reason: Refusal, owner: u32, tag: &str, recorded: Option<LayerPainter>) {
    crate::why::note("not_own", || {
        format!(
            "{}: box {owner} <{tag}>, recorded painter {recorded:?}",
            reason.label()
        )
    });
}

/// Count a refusal of a box whose own paint the layer does not hold.
#[track_caller]
pub(super) fn refuse_not_own(reason: Refusal) {
    crate::paint_path::refuse_with(&crate::paint_path::REFUSED_NOT_OWN, || {
        reason.label().to_string()
    });
    crate::paint_path::not_own::add(reason.label());
}
