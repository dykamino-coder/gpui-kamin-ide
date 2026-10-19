//! Select the containing block for a replaced inline box with one explicit inset axis.

use crate::layout::positioned::containing_block::{Positioning, SpotCell};
use crate::style::computed::{Computed, Position};
use gpui::AnyElement;

pub(crate) fn push(
    spot: SpotCell,
    element: AnyElement,
    own: &Computed,
    inherited: &Computed,
    positioning: &Positioning,
) -> Option<AnyElement> {
    // CSS 2.1 §10.1: without a positioned ancestor an absolute box uses the
    // initial containing block. The spot still supplies its static coordinate
    // on the auto axis (§§10.3.7, 10.6.4); an inline wrapper must not become
    // the containing block just because only one inset axis is explicit.
    if own.position != Some(Position::Absolute) {
        return positioning.late_push(spot, element);
    }
    if inherited.cb_ancestor || crate::text::inline::establishes_cb(inherited) {
        positioning.cb_push(
            spot,
            element,
            crate::layout::positioned::containing_block::nearest_cb_index(inherited),
        )
    } else {
        positioning.icb_push(spot, element)
    }
}
