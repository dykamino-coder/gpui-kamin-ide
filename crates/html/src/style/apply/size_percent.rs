//! Minimum calc sizes retain their length term when percentages are cyclic.
//! CSS Sizing 3 §5.2.1 resolves cyclic percentages in minimum contributions to zero.

use crate::style::computed::{Computed, Position};
use crate::style::values::value::{Len, calc_get};

pub(super) fn resolve(style: &Computed, value: Len, field: u8) -> Option<Len> {
    let (percentage, length) = match value {
        Len::Pct(p) => (p, 0.0),
        Len::Calc(index) => match calc_get(index).pct_px() {
            Some(terms) => terms,
            None => return Some(value),
        },
        Len::Px(length) => return Some(Len::Px(length.max(0.0))),
        _ => return Some(value),
    };
    // A zero percentage cannot contribute to a minimum, even when its basis
    // is indefinite. Resolving its length here also preserves intrinsic minima.
    if matches!(field, 2 | 3) && percentage == 0.0 && length != 0.0 {
        return Some(Len::Px(length.max(0.0)));
    }
    if field.is_multiple_of(2)
        || style.cb_height_def
        || style.root_box
        || matches!(style.position, Some(Position::Absolute | Position::Fixed))
    {
        return Some(value);
    }
    // Height and maximum height become auto; minimum height keeps the fixed
    // term instead of discarding the entire length-percentage expression.
    (field == 3 && length != 0.0).then_some(Len::Px(length.max(0.0)))
}
