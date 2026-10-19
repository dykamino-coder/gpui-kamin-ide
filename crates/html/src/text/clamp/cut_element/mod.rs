//! Clamp probe element and its layout, prepaint, and plan application lifecycle.

mod layout;
mod painting;
mod prepaint;

use super::{ClampEntry, ClampLines, clamp_cut, clamp_para};
use crate::hooks::Hooks;
use gpui::{App, Bounds, LayoutId, Pixels, Window};

/// Вычислитель точки среза: абсолютный элемент В КОНЦЕ clamp-контейнера,
/// его границы — весь контейнер. Считает низ N-й считаемой строки,
/// поднимает срез к верху пересечённого блока и просит новый кадр, когда
/// точка изменилась.
pub struct ClampCut {
    pub(crate) values: crate::document::state::ValueDoc,
    pub(crate) rows: std::rc::Rc<super::ParagraphRows>,
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

impl Hooks for ClampCut {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        self.request_layout_impl(window, cx)
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::Prepaint {
        self.prepaint_impl(_bounds, _state, _window, _cx)
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        _cx: &mut App,
    ) {
        self.paint_impl(bounds, _state, _prepaint, window, _cx)
    }
}

impl ClampCut {
    pub fn new(
        key: u64,
        lines: ClampLines,
        limit: Option<u32>,
        max_h: Option<f32>,
        cx: &crate::layout::cx::LayoutCx,
    ) -> Self {
        ClampCut {
            values: cx.vcx().keep(),
            rows: Default::default(),
            key,
            lines,
            limit,
            max_h,
            trim_end: 0.0,
        }
    }
}

impl ClampCut {
    /// Share the row owner of the paragraphs in this document.
    pub(crate) fn measure_context(mut self, cx: &crate::layout::cx::LayoutCx) -> Self {
        self.rows = cx.doc().paragraph_rows.clone();
        self
    }

    /// Срез конца последней видимой строки (`text-box-trim`).
    pub fn trim_end(mut self, v: f32) -> Self {
        self.trim_end = v.max(0.0);
        self
    }
}
