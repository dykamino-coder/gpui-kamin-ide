//! Подложка `::first-line` под фоном строчных коробок первой строки: фон псевдоэлемента —
//! нижний слой, а не замена фона потомков (css-pseudo-4 §2.1.3), поэтому цвет полосы прогона
//! — композиция фона куска поверх фона первой строки.

use crate::style::values::value::Color;

pub(super) fn paint_color(child: Option<Color>, first_line: Option<Color>) -> Option<Color> {
    // CSS Pseudo 4 section 2.1.3: non-inherited properties inherit from the
    // non-pseudo parent. The first-line background is a lower paint layer,
    // rather than an override of backgrounds on its inline descendants.
    match (child, first_line) {
        (Some(child), Some(parent)) => Some(crate::style::values::composite::over(child, parent)),
        (child, None) => child,
        (None, parent) => parent,
    }
}
