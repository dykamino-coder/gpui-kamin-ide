//! Применение к GPUI: процентные вставки `top`/`bottom` у `position: relative`
//! при неопределённой высоте содержащего блока считаются `auto` (CSS 2 §9.3.2).
//! Доля внутри `calc()` сохраняет свою базу до раскладки (CSS Values 4 §10.9),
//! поэтому предикат смотрит и на процентную часть смешанной длины.

use crate::document::state::ValueCx;
use crate::style::computed::{Computed, Position};
use crate::style::values::value::{Len, calc_get};

pub(super) fn is_auto(style: &Computed, value: Len, side: u8, vcx: ValueCx<'_>) -> bool {
    let percentage = match value {
        Len::Pct(_) => true,
        Len::Calc(index) => calc_get(index, vcx).pct != 0.0,
        _ => false,
    };
    style.position == Some(Position::Relative)
        && matches!(side, 0 | 2)
        && !style.cb_height_def
        && percentage
}
