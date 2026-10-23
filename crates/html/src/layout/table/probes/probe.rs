//! Проба ячейки снимает маску и точную область фона до paint того же кадра.

use super::RowRects;
use gpui::{AnyElement, Bounds, Pixels, px};

/// CSS 2.1 §17.5.1: фон полосы идёт до внешних рамочных краёв ячейки,
/// хотя абсолютная проба занимает только её padding box.
pub fn cell_rect_probe(
    rects: RowRects,
    exact: bool,
    shift: (f32, f32),
    border: [f32; 4],
) -> AnyElement {
    rects.record_with(move |g, _| {
        let outer = |bounds: Bounds<Pixels>| Bounds {
            origin: gpui::point(
                bounds.origin.x + px(shift.0 - border[3]),
                bounds.origin.y + px(shift.1 - border[0]),
            ),
            size: gpui::size(
                bounds.size.width + px(border[1] + border[3]),
                bounds.size.height + px(border[0] + border[2]),
            ),
        };
        // Перемещённые фрагменты имеют другую систему отсчёта; в ней
        // неокруглённая геометрия исходной раскладки больше неприменима.
        (outer(g.bounds), exact, outer(g.aligned(px(1.))))
    })
}
