//! Применение к GPUI: процентные размеры по блочной оси при неопределённой
//! высоте содержащего блока. `height`/`max-height` становятся `auto`, а
//! `min-height` в `calc()` сохраняет точечное слагаемое вместо отброса всего
//! выражения: циклический процент в минимальном вкладе разрешается в ноль
//! (CSS Sizing 3 §5.2.1). Нулевой процент разрешается всегда.

use crate::document::state::ValueCx;
use crate::style::computed::{Computed, Position};
use crate::style::values::value::{Len, calc_get};

pub(super) fn resolve_cyclic_size_percentage(
    style: &Computed,
    value: Len,
    field: u8,
    vcx: ValueCx<'_>,
) -> Option<Len> {
    let (percentage, length) = match value {
        Len::Pct(p) => (p, 0.0),
        Len::Calc(index) => match calc_get(index, vcx).pct_px() {
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
