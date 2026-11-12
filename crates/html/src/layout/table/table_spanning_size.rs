//! Заявленный минимум охватывающей ячейки при авто-раскладке таблицы до расчёта сетки:
//! авторская `width` поднимает минимум ячейки (CSS 2.1 §17.5.2.2) и её колонок вместе,
//! без двойного учёта внутренних зазоров при раздаче по колонкам.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// The `min-width` a spanning cell of merged style `merged` takes, if any.
pub(super) fn minimum(
    merged: &Computed,
    span: u16,
    fixed: bool,
    vertical: bool,
    collapsed: bool,
) -> Option<Len> {
    if span <= 1 || fixed || vertical || collapsed || merged.wm().vertical() {
        return None;
    }
    // CSS 2.1 §17.5.2.2: an authored width raises the cell minimum, and a
    // spanning cell raises its columns together. Grid measurement includes
    // internal gutters in this minimum; assigning each column the width would
    // count those gutters twice. Percentages need the table's resolved basis.
    let Some(Len::Px(width)) = merged.reset_box_layout.width else {
        return None;
    };
    Some(Len::Px(match merged.reset_box_layout.min_width {
        Some(Len::Px(minimum)) => width.max(minimum),
        _ => width,
    }))
}
