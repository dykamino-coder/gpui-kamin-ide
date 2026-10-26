//! Вычисление линейки колонок перед построением рядов.

use crate::style::computed::BoxStyle;

/// Вычисленная линейка, общая для всех рядов.
pub(super) fn column_rule(
    e: &crate::dom::Element,
    inherited: &BoxStyle,
    size: f32,
) -> Option<(f32, crate::style::values::value::Color)> {
    let style = e.style.used();
    (style.column_rule_visible == Some(true)).then(|| {
        let width = match style.column_rule_width {
            Some(crate::style::values::value::Len::Px(v)) => v,
            Some(crate::style::values::value::Len::Em(k)) => k * size,
            _ => 3.,
        };
        (
            width,
            style.column_rule_color.resolve(
                inherited
                    .color
                    .unwrap_or(crate::style::values::value::Color::BLACK),
            ),
        )
    })
}
