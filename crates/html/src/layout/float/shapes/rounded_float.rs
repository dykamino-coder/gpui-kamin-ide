//! Скруглённая коробка обтекания из `shape-outside` (`inset()`/`rect()`/`xywh()` с `round` и
//! слова-коробки, css-shapes-1 §3.1): непрерывный `RoundedBox` вместо растрового профиля.

use crate::document::state::ValueCx;
use crate::style::values::basic_shape::{ShapeBox, rrect_of};

pub fn rounded_float(
    raw: &str,
    b: &ShapeBox,
    side: i32,
    vcx: ValueCx<'_>,
) -> Option<crate::layout::float::rounded_box::RoundedBox> {
    let (rect, radii) = rrect_of(raw, b, vcx)?;
    Some(crate::layout::float::rounded_box::RoundedBox::new(
        rect,
        radii,
        (b.mw, b.mh),
        side,
    ))
}
