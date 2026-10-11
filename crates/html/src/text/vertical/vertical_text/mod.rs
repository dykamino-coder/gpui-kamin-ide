//! Vertical text for vertical; split out to keep the owning module within 250 lines.

mod layout;
mod painting;
mod prepaint;

use super::{VT_FRAME, VT_INLINE_MAX, VT_MEASURED};
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};

pub struct VerticalText {
    pub(crate) child: Option<AnyElement>,
    /// Естественный размер содержимого до поворота.
    pub(crate) natural: gpui::Size<Pixels>,
    /// Потолок заявляемой высоты (см. `claiming_height`).
    pub(crate) claim_cap: Option<Pixels>,
    /// Ключ двухкадрового замера (текст абзаца + соль документа).
    pub(crate) key: Option<u64>,
    /// Предел строки от родителя: если строка УЖЕ помещается, высота
    /// заявляется честно — иначе гибкая ячейка считает коробку нулевой и
    /// `justify-content` уводит рисунок из виду (table-cell-align-005).
    pub(crate) fit_limit: Option<Pixels>,
    pub(crate) inline_constraint: Option<crate::style::computed::orthogonal::InlineConstraint>,
    pub(crate) inline_keyword: Option<crate::style::computed::orthogonal::InlineKeyword>,
    /// `writing-mode: sideways-lr` — поворот ПРОТИВ часовой стрелки.
    /// css-writing-modes-4, таблица Abstract-Physical Mapping: у `sideways-lr`
    /// line-left = НИЗ, line-right = ВЕРХ, over = ЛЕВО (у всех остальных
    /// вертикальных письмён line-left = верх, over = право). Blink различает
    /// эти два случая ровно так же — `paint/line_relative_rect.cc:69-75`:
    /// `AffineTransform(0, 1, -1, 0, …)` против `AffineTransform(0, -1, 1, 0, …)`.
    pub(crate) ccw: bool,
    /// Ячейка вертикальной таблицы: мерить содержимое по МИНИМАЛЬНОМУ
    /// вдоль строки, а не по максимальному. Тогда заявленная высота
    /// повёрнутой коробки — вклад ячейки в меру её КОЛОНКИ (css-tables-3
    /// §computing-column-measures), и дорожку считает решётка, а не
    /// инлайн-размер всего стола. Ставится из `render.rs` (`col_min`).
    pub(crate) col_min: bool,
    /// `vertical-lr` при повороте по часовой: строки поданы снизу вверх, и
    /// первая строка — ЛЕВАЯ колонка (у `vertical-rl` — правая).
    pub(crate) lr: bool,
    /// Первая строка для базовой по оси x: шрифт, кегль, высота строки
    /// (`None` — `normal`) и центральная ли доминантная базовая.
    pub(crate) first_line: Option<(gpui::Font, Pixels, Option<Pixels>, bool)>,
}

impl Element for VerticalText {
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
        self.request_layout_impl(_id, _inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.prepaint_impl(_id, _inspector_id, bounds, _state, window, cx)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.paint_impl(_id, _inspector_id, bounds, _state, _prepaint, window, cx)
    }
}

impl IntoElement for VerticalText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
