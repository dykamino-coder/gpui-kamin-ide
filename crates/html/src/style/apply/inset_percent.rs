//! Percentage insets retain their basis through calc() until layout.
//!
//! CSS Values 4 §10.9 preserves mixed lengths; CSS 2 §9.3.2 makes vertical
//! relative percentage insets auto when the containing height is indefinite.

use crate::computed::{Computed, Position};
use crate::value::{Len, calc_get};

pub(super) fn is_auto(style: &Computed, value: Len, side: u8) -> bool {
    let percentage = match value {
        Len::Pct(_) => true,
        Len::Calc(index) => calc_get(index).pct != 0.0,
        _ => false,
    };
    style.position == Some(Position::Relative)
        && matches!(side, 0 | 2)
        && !style.cb_height_def
        && percentage
}
