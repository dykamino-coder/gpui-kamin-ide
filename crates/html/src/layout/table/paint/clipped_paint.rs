//! Отрисовка полосы ряда/группы, обрезанной ячейками: тени, контур и слой фона (gpui Element).

use super::CellsClipped;
use crate::hooks::Hooks;
use gpui::{App, Bounds, LayoutId, Pixels, Window};

impl Hooks for CellsClipped {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let mut style = gpui::Style::default();
        // Оверлей вне потока: раскладку таблицы полоса не трогает.
        style.position = gpui::Position::Absolute;
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
        // Пробы ячеек пишут в PREPAINT, а вся подготовка кадра идёт до
        // отрисовки — здесь забираются прямоугольники ЭТОГО ЖЕ кадра. Пусто —
        // у полосы нет ячеек (`<col>` без рядов, `<col>` за краем сетки).
        let rects = std::mem::take(&mut *self.rects.borrow_mut());
        if rects.is_empty() {
            return;
        }
        // Область ряда/колонки — охват ТОЧНЫХ ячеек (span = 1): от неё
        // считается и размер плитки, и `background-position`. Объединённые
        // лежат и на чужих дорожках — они только маски.
        let union = |pick: &dyn Fn(&(Bounds<Pixels>, bool, Bounds<Pixels>)) -> Bounds<Pixels>| {
            let exact: Vec<Bounds<Pixels>> = rects.iter().filter(|r| r.1).map(pick).collect();
            let all: Vec<Bounds<Pixels>> = rects.iter().map(pick).collect();
            let base = if exact.is_empty() { all } else { exact };
            let mut area = base[0];
            for r in &base[1..] {
                let right = area.origin.x + area.size.width;
                let bottom = area.origin.y + area.size.height;
                let x0 = area.origin.x.min(r.origin.x);
                let y0 = area.origin.y.min(r.origin.y);
                let x1 = right.max(r.origin.x + r.size.width);
                let y1 = bottom.max(r.origin.y + r.size.height);
                area = Bounds {
                    origin: gpui::point(x0, y0),
                    size: gpui::size(x1 - x0, y1 - y0),
                };
            }
            area
        };
        // Тень и обводка — по округлённым ячейкам (резкие края); фон
        // позиционируется по неокруглённым (CSS 2.1 §17.5.1, как у Blink).
        let area = union(&|r| r.0);
        let positioning = union(&|r| r.2);
        let all: Vec<Bounds<Pixels>> = rects.iter().map(|(b, _, _)| *b).collect();
        // Тень РЯДА — вокруг охвата всех его ячеек, без маски: она лежит
        // снаружи. Резкая (без размытия) рисуется кольцевым квадом — тот же
        // обход вырождения шейдера, что у обычных коробок.
        self.paint_band_shadows(window, area);
        // Обводка ряда/группы — вокруг охвата ТОЧНЫХ ячеек, снаружи и без
        // маски (css-ui-4 §outline: рамка вне коробки, раскладку не трогает).
        // Тот же кольцевой квад, что у резкой тени выше.
        self.paint_band_outline(window, area);
        for rect in all {
            window.with_content_mask(Some(gpui::ContentMask { bounds: rect }), |window| {
                // Цвет ряда — под картинкой, в тех же прямоугольниках: на
                // ячейки его в этом случае не переносят (иначе он закрашивал
                // бы картинку, рисуясь позже полосы).
                if let Some(bg) = self.style.background {
                    window.paint_quad(gpui::fill(rect, bg.to_hsla()));
                }
                crate::paint::background::paint_area(&self.style, positioning, window);
            });
        }
    }
}
