//! Preserve an auto-layout spanning cell's declared minimum before grid sizing.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

pub(super) fn preserve(
    cell: &mut Computed,
    merged: &Computed,
    span: u16,
    fixed: bool,
    vertical: bool,
    collapsed: bool,
) {
    if span <= 1 || fixed || vertical || collapsed || merged.vertical == Some(true) {
        return;
    }
    // CSS 2.1 §17.5.2.2: an authored width raises the cell minimum, and a
    // spanning cell raises its columns together. Grid measurement includes
    // internal gutters in this minimum; assigning each column the width would
    // count those gutters twice. Percentages need the table's resolved basis.
    if let Some(Len::Px(width)) = merged.width {
        cell.min_width = Some(Len::Px(match merged.min_width {
            Some(Len::Px(minimum)) => width.max(minimum),
            _ => width,
        }));
    }
}
