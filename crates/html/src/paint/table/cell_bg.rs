//! Слой фонов ячеек сросшейся таблицы: красит прямоугольники, снятые пробами ячеек
//! (`layout::table::probes::cell_bg_probe`) на подготовке того же кадра.

use super::LayerPaint;
use crate::layout::table::probes::CellBgs;
use gpui::Window;

/// Слой фонов ячеек сросшейся таблицы (см. `CellBgs`).
pub struct CellBgPainter {
    pub(crate) bgs: CellBgs,
}

impl CellBgPainter {
    pub fn new(bgs: CellBgs) -> Self {
        CellBgPainter { bgs }
    }
}

impl LayerPaint for CellBgPainter {
    fn paint(&mut self, window: &mut Window) {
        // Пробы пишут на подготовке, вся подготовка кадра идёт до отрисовки —
        // здесь прямоугольники ЭТОГО ЖЕ кадра.
        let bgs = std::mem::take(&mut *self.bgs.borrow_mut());
        for (bounds, colour) in bgs {
            window.paint_quad(gpui::fill(bounds, colour));
        }
    }
}
