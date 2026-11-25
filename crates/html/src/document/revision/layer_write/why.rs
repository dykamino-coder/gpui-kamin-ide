//! The DevTools `why` record of a layer write the layer refuses: which box,
//! which painter it was recorded with and why it cannot take the new paint.

use crate::paint::box_tree::styled::Refusal;
use gpui::LayerPainter;

/// `not_own`: box `owner` paints what the layer cannot take (`reason`); a box
/// without a run names why the layer kept none (`gpui::LayerHandle::no_run`).
#[track_caller]
pub(super) fn not_own(
    reason: Refusal,
    owner: u32,
    tag: &str,
    layer: &gpui::LayerHandle,
    recorded: Option<LayerPainter>,
    (damage, wanted): (super::PaintDamage, crate::paint::box_tree::styled::Own),
) {
    crate::why::note("not_own", || {
        let run = match recorded {
            Some(_) => format!(" (runs: {:?})", layer.run_painters(owner)),
            None => format!(
                " ({})",
                layer
                    .no_run(owner)
                    .unwrap_or("no run refused: the frame did not paint the box")
            ),
        };
        format!(
            "{}: box {owner} <{tag}>, recorded painter {recorded:?}, new {wanted:?}, {damage:?}{run}",
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
