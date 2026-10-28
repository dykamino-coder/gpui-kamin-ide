//! Точки разрыва детей, чья колонка известна только после раскладки коробки стопки.

use crate::layout::fragment::types::BreaksAt;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, Window, px};

impl ColumnStack {
    /// Измеренные точки разрыва детей, чья колонка до раскладки коробки
    /// стопки неизвестна (`BreaksAt::Column`): главная копия раскладывается
    /// шириной `col_w`, и её строки и блочная стопка заменяют оценку. Только
    /// при высоте колонки, заданной до меры детей (`fixed_height`): высоту
    /// балансируемой коробки решила мера при запросе раскладки.
    pub(crate) fn column_breaks(&mut self, col_w: f32, window: &mut Window, cx: &mut App) {
        if self.fixed_height.is_none() {
            return;
        }
        for c in self.children.iter_mut() {
            if c.measure.is_some() || c.span || c.breaks_w != Some(BreaksAt::Column) {
                continue;
            }
            let sz = c.el.layout_as_root(
                gpui::size(
                    gpui::AvailableSpace::Definite(px(col_w)),
                    gpui::AvailableSpace::MaxContent,
                ),
                window,
                cx,
            );
            #[cfg(feature = "wpt-harness")]
            if c.probe_pending && c.watched.is_some() {
                c.probe.height(
                    "columns.column",
                    c.h,
                    f32::from(if self.axis.is_vertical() {
                        sz.width
                    } else {
                        sz.height
                    }),
                );
            }
            c.replace_breaks(sz, self.axis, window);
        }
    }
}
