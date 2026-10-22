//! Слой фонов ячеек сросшейся таблицы: красит прямоугольники, снятые пробами ячеек
//! (`layout::table::probes::cell_bg_probe`) на подготовке того же кадра.

use crate::hooks::Hooks;
use crate::layout::table::probes::CellBgs;
use gpui::{App, Bounds, LayoutId, Pixels, Window};

/// Слой фонов ячеек сросшейся таблицы (см. `CellBgs`).
pub struct CellBgPainter {
    pub(crate) bgs: CellBgs,
}

impl CellBgPainter {
    pub fn new(bgs: CellBgs) -> Self {
        CellBgPainter { bgs }
    }
}

impl Hooks for CellBgPainter {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let style = gpui::Style {
            position: gpui::Position::Absolute,
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::Prepaint {
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        _cx: &mut App,
    ) {
        // Пробы пишут на подготовке, вся подготовка кадра идёт до отрисовки —
        // здесь прямоугольники ЭТОГО ЖЕ кадра.
        let bgs = std::mem::take(&mut *self.bgs.borrow_mut());
        for (bounds, colour) in bgs {
            window.paint_quad(gpui::fill(bounds, colour));
        }
    }
}
