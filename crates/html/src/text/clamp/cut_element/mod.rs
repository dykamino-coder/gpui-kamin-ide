//! Cut element for clamp; split out to keep the owning module within 250 lines.

mod layout;
mod painting;
mod prepaint;

use super::{ClampEntry, ClampLines, clamp_cut, clamp_para, take_para_rows};
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Window,
};

/// Вычислитель точки среза: абсолютный элемент В КОНЦЕ clamp-контейнера,
/// его границы — весь контейнер. Считает низ N-й считаемой строки,
/// поднимает срез к верху пересечённого блока и просит новый кадр, когда
/// точка изменилась.
pub struct ClampCut {
    pub(crate) key: u64,
    pub(crate) lines: ClampLines,
    /// Число считаемых строк; None — `line-clamp: auto` (срез только по
    /// потолку высоты, но пересечённый блок всё равно прячется целиком).
    pub(crate) limit: Option<u32>,
    /// Потолок высоты контейнера в точках (max-height), если задан.
    pub(crate) max_h: Option<f32>,
    /// `text-box-trim: trim-end` контейнера в точках: последняя строка
    /// ПЕРЕД точкой обрыва — последняя отформатированная, и её конец
    /// срезается (`text-box-trim-line-clamp-*`). Ноль — среза нет.
    pub(crate) trim_end: f32,
}

impl Element for ClampCut {
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
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
        self.prepaint_impl(_id, _inspector_id, _bounds, _state, _window, _cx)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        self.paint_impl(_id, _inspector_id, bounds, _state, _prepaint, window, _cx)
    }
}

impl IntoElement for ClampCut {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl ClampCut {
    pub fn new(key: u64, lines: ClampLines, limit: Option<u32>, max_h: Option<f32>) -> Self {
        ClampCut {
            key,
            lines,
            limit,
            max_h,
            trim_end: 0.0,
        }
    }
}

impl ClampCut {
    /// Срез конца последней видимой строки (`text-box-trim`).
    pub fn trim_end(mut self, v: f32) -> Self {
        self.trim_end = v.max(0.0);
        self
    }
}
