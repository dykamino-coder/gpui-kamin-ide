//! Select the containing block for a replaced inline box with one explicit inset axis.

use crate::computed::{Computed, Position};
use crate::interact::{SpotCell, cb_push, icb_push, late_push};
use gpui::AnyElement;

pub(super) fn push(
    spot: SpotCell,
    element: AnyElement,
    own: &Computed,
    inherited: &Computed,
) -> Option<AnyElement> {
    // CSS 2.1 §10.1: without a positioned ancestor an absolute box uses the
    // initial containing block. The spot still supplies its static coordinate
    // on the auto axis (§§10.3.7, 10.6.4); an inline wrapper must not become
    // the containing block just because only one inset axis is explicit.
    if own.position != Some(Position::Absolute) {
        return late_push(spot, element);
    }
    if inherited.cb_ancestor || crate::inline::establishes_cb(inherited) {
        cb_push(spot, element)
    } else {
        icb_push(spot, element)
    }
}
