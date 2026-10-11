//! Проба прямоугольника ячейки: копит границы ячеек кадра для полос рядов и колонок.

use super::RowRects;
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Styled, Window, px,
};

/// Проба ячейки: канвас, записывающий свои границы для фона ряда.
///
/// `border` — ширины сторон ячейки в порядке верх-право-низ-лево. Абсолютный
/// ребёнок в taffy лежит ВНУТРИ рамки, поэтому канвас меряет поле подкладки,
/// а §17.5.1 велит вести фон полосы «from the top of the cells to the bottom
/// of the cells», то есть по внешним краям рамок: ячейка с
/// `border-bottom: 60px` и пустым содержимым давала полосе нулевую высоту.
pub fn cell_rect_probe(
    rects: RowRects,
    exact: bool,
    shift: (f32, f32),
    border: [f32; 4],
) -> AnyElement {
    CellProbe {
        child: Some(
            gpui::div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .into_any_element(),
        ),
        rects,
        exact,
        shift,
        border,
    }
    .into_any_element()
}

/// Проба ячейки (`cell_rect_probe`): записывает и округлённый прямоугольник
/// (маски краски), и неокруглённый (область позиционирования фона полосы).
/// Поле подкладки ячейки округляется от её внутреннего края рамки: 25px
/// рамки при 1.25 сдвигали его на 0.2px, и плитка `top right` у tbody
/// вставала на точку правее эталона (`background-position-applies-to-001a`).
pub(super) struct CellProbe {
    pub(crate) child: Option<AnyElement>,
    pub(crate) rects: RowRects,
    pub(crate) exact: bool,
    pub(crate) shift: (f32, f32),
    pub(crate) border: [f32; 4],
}

impl CellProbe {
    pub(crate) fn outer(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        let (shift, border) = (self.shift, self.border);
        // Сдвиг краски относительно коробки ячейки: в сросшейся модели
        // фоновая сетка начинается от середины рамки таблицы.
        Bounds {
            origin: gpui::point(
                bounds.origin.x + gpui::px(shift.0 - border[3]),
                bounds.origin.y + gpui::px(shift.1 - border[0]),
            ),
            size: gpui::size(
                bounds.size.width + gpui::px(border[1] + border[3]),
                bounds.size.height + gpui::px(border[0] + border[2]),
            ),
        }
    }
}

impl Element for CellProbe {
    type RequestLayoutState = LayoutId;
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
    ) -> (LayoutId, LayoutId) {
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Запись В PREPAINT: подготовка ВСЕХ элементов идёт до отрисовки,
        // и полоса фона читает прямоугольники СВОЕГО кадра — с записью в
        // paint она рисовала прошлый кадр и мигала на каждой смене раскладки.
        let unrounded = Bounds {
            origin: window.layout_origin_unrounded(*layout_id),
            size: window.layout_size_unrounded(*layout_id),
        };
        // Неокруглённое берётся, только пока оно в пределах точки
        // устройства от округлённого (иначе — другой кадр отсчёта).
        let near = |a: Pixels, b: Pixels| (a - b).abs() <= px(1.0);
        let unrounded = if near(unrounded.left(), bounds.left())
            && near(unrounded.top(), bounds.top())
            && near(unrounded.right(), bounds.right())
            && near(unrounded.bottom(), bounds.bottom())
        {
            unrounded
        } else {
            bounds
        };
        self.rects
            .borrow_mut()
            .push((self.outer(bounds), self.exact, self.outer(unrounded)));
        self.child.as_mut().unwrap().prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.as_mut().unwrap().paint(window, cx);
    }
}

impl IntoElement for CellProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
