//! Отрисовка полосы ряда/группы, обрезанной ячейками: тени, контур и слой фона (gpui Element).

use super::{BAND_WAIT_FRAMES, CellsClipped};
use crate::layout::table::paint::BAND_RETRIES;
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, LayoutId, Pixels, Window,
};

impl Element for CellsClipped {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = gpui::Style::default();
        // Оверлей вне потока: раскладку таблицы полоса не трогает.
        style.position = gpui::Position::Absolute;
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        // Пробы ячеек пишут в PREPAINT, а вся подготовка кадра идёт до
        // отрисовки — здесь забираются прямоугольники ЭТОГО ЖЕ кадра.
        // Пустота возможна только на самом первом кадре документа.
        let rects = std::mem::take(&mut *self.rects.borrow_mut());
        if rects.is_empty() {
            // Пробы ячеек ещё не писали (первый кадр) — без нового кадра
            // окно не перерисуется, и фон не появится никогда. Но ждать
            // бесконечно нельзя: у полосы может не быть ячеек вовсе.
            let key = std::rc::Rc::as_ptr(&self.rects) as usize;
            let waited = BAND_RETRIES.with(|m| {
                let mut m = m.borrow_mut();
                let n = m.entry(key).or_insert(0);
                *n = n.saturating_add(1);
                *n
            });
            if waited <= BAND_WAIT_FRAMES {
                window.request_animation_frame();
            }
            return;
        }
        BAND_RETRIES.with(|m| {
            m.borrow_mut()
                .remove(&(std::rc::Rc::as_ptr(&self.rects) as usize));
        });
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
