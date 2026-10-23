//! Заданная высота ряда колонок многоколоночника.

use crate::dom::Element;
use crate::style::values::value::Len;

pub(super) fn fixed_column_height(
    e: &Element,
    col_vert: bool,
    col_h: Option<f32>,
    nest_rows: Option<f32>,
) -> Option<f32> {
    if let Some(hh) = nest_rows {
        (e.style.used().column_fill_auto == Some(true)).then_some(hh)
    } else if e.style.used().column_fill_auto == Some(true) {
        col_h.or(
            match if col_vert {
                e.style.used().width
            } else {
                e.style.used().height
            } {
                Some(Len::Px(h)) => Some(h),
                // Высота авто при заданном `max-height`: колонки заполняются
                // подряд до него (css-multicol-1 §column-fill `auto`; Blink
                // `ConstrainColumnBlockSize` берёт `LogicalMaxHeight()`), как
                // в текстовом пути `column_flow` (`columnfill-auto-max-height-003`).
                None | Some(Len::Auto) if !col_vert => match e.style.used().max_height {
                    Some(Len::Px(m)) if m > 0.0 => Some(m),
                    _ => None,
                },
                _ => None,
            },
        )
    } else {
        None
    }
}
