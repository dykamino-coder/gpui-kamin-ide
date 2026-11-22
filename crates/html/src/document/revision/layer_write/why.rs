//! The DevTools `why` records of a layer write the layer refuses: which box,
//! which painter it was recorded with and what it paints now.

use crate::paint::box_tree::styled::{Own, Refusal};
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

/// `not_own.kind`: a changed box kind or a box the layer never recorded.
#[track_caller]
pub(super) fn not_own_kind(owner: u32, tag: &str, recorded: Option<LayerPainter>, now: Own) {
    crate::why::note("not_own.kind", || match recorded {
        None => format!("box {owner} <{tag}> has no painter in the layer; new paint {now:?}"),
        Some(p) => format!("box {owner} <{tag}>: recorded painter {p:?}, new paint {now:?}"),
    });
}
