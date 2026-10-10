//! grid_style, контейнер лунок на пути сетки: порог flow-tolerance, упаковка и направление лунок для taffy.

use super::*;

pub(super) fn grid_lanes(mut d: Div, c: &Computed) -> Div {
    // Контейнер лунок на пути сетки (`dom::lanes_as_grid`): раскладку лунками
    // делает taffy. Порог `flow-tolerance: normal` — 1em (css-grid-3
    // Overview.bs:828-831), `infinite` разбор держит бесконечными точками.
    if c.lanes_taffy {
        let em = match c.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let (tolerance, tolerance_pct) = match c.lanes_tolerance {
            Some(Len::Px(v)) => (v, None),
            Some(Len::Em(k)) => (k * em, None),
            Some(Len::Pct(k)) => (0.0, Some(k)),
            _ => (em, None),
        };
        // Ось решётки taffy — ФИЗИЧЕСКАЯ, а `grid-lanes-direction` —
        // логическая: при вертикальном письме колонки идут по y, ряды — по x
        // (та же перестановка, что у дорожек сетки ниже, `flip`).
        // ★ ЗАМЕРЕНО: `vertical-rl` как `fill-reverse` колоночных лунок —
        // лунки вертикального письма (116 пар) +1/−1, не взято.
        let vertical = c.vertical == Some(true);
        let logical_rows = crate::dom::lanes_row_dir(c);
        d.style().grid_lanes = Some(gpui::GridLanesFlow {
            rows: logical_rows != vertical,
            track_reverse: c.lanes_track_reverse,
            fill_reverse: c.lanes_fill_reverse,
            dense: c.lanes_dense,
            tolerance,
            tolerance_pct,
            stack_block: vertical && !logical_rows,
        });
        // По оси укладки `normal` — это НЕ растяжка (css-grid-3
        // Overview.bs:1161-1225: самовыравнивание лишь у элементов над
        // проёмом; Blink `ResolvedAlignSelf(normal)` :1056-1060), а общий
        // путь `align-items: stretch` в стиль не пишет — для сетки это
        // умолчание. Лункам явная растяжка нужна в стиле.
        if c.align_items == Some(crate::style::computed::Align::Stretch) {
            d.style().align_items = Some(gpui::AlignItems::Stretch);
        }
    }
    d
}
