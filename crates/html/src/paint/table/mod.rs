//! Слои покраски таблицы по геометрии, снятой пробами раскладки (`layout::table::probes`):
//! полосы рядов, групп и колонок, обрезанные ячейками (`CellsClipped`), фоны ячеек сросшейся
//! модели (`CellBgPainter`) и сросшиеся кромки (`EdgePainter`, CSS 2.1 §17.6.2.1).

mod cell_bg;
mod clipped;
mod clipped_paint;
mod edge_painter;
mod edge_segments;
pub use cell_bg::CellBgPainter;
pub use clipped::CellsClipped;
pub use edge_painter::EdgePainter;

use crate::hooks::{Hooks, IntoHooked};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window};

/// Покраска слоя таблицы в `paint` того кадра, чьи пробы записали геометрию в `prepaint`.
pub(crate) trait LayerPaint: 'static {
    fn paint(&mut self, window: &mut Window);
}

/// Элемент слоя: оверлей вне потока без детей — раскладку таблицы он не трогает, на
/// подготовке ничего не делает и только красит.
pub(crate) fn layer(paint: impl LayerPaint) -> AnyElement {
    Layer(paint).hooked()
}

struct Layer<T>(T);

impl<T: LayerPaint> Hooks for Layer<T> {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let style = gpui::Style {
            position: gpui::Position::Absolute,
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {}

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        self.0.paint(window);
    }
}
