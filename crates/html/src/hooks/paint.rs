//! Адаптация CSS-краски для consumers фаз раскладки и текста.

use gpui::{AnyElement, Div, IntoElement, Window};

pub(crate) fn paint_root(child: AnyElement) -> AnyElement {
    crate::paint::coordinator::roots(vec![child])
        .pop()
        .expect("один корень")
}

pub(crate) fn paint_last(child: AnyElement, key: u64) -> AnyElement {
    crate::paint::coordinator::PaintLast::new(child)
        .key(key)
        .into_any_element()
}

pub(crate) fn paint_atomic(div: Div) -> Div {
    crate::paint::coordinator::atomic(div)
}

pub(crate) fn paint_reopen<R>(window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
    crate::paint::coordinator::reopen(window, f)
}

pub(crate) use crate::layout::fragment::paint_source::{BareRecords, SourceSlices};

pub(crate) fn prepaint_source_record(
    extent: gpui::Bounds<gpui::Pixels>,
    segments: std::rc::Rc<[gpui::PaintSegment]>,
    slices: SourceSlices,
    window: &mut Window,
    cx: &mut gpui::App,
    prepaint: impl FnOnce(&mut Window, &mut gpui::App),
) -> Result<(), gpui::PaintRecordError> {
    crate::paint::coordinator::segments::prepaint(extent, segments, slices, window, cx, prepaint)
}

pub(crate) fn paint_source_record(
    extent: gpui::Bounds<gpui::Pixels>,
    segments: std::rc::Rc<[gpui::PaintSegment]>,
    slices: SourceSlices,
    window: &mut Window,
    cx: &mut gpui::App,
    paint: impl FnOnce(&mut Window, &mut gpui::App),
) -> Result<(gpui::PaintSegmentRecords, BareRecords), gpui::PaintRecordError> {
    crate::paint::coordinator::segments::record(extent, segments, slices, window, cx, paint)
}

pub(crate) fn paint_source_node(
    id: u64,
    cb: Option<u64>,
    placed: gpui::Point<gpui::Pixels>,
    window: &mut Window,
    cx: &mut gpui::App,
    paint: impl FnOnce(&mut Window, &mut gpui::App),
) {
    crate::paint::coordinator::segments::node(id, cb, placed, window, cx, paint);
}

#[cfg(test)]
pub(crate) fn paint_inline(child: AnyElement) -> AnyElement {
    crate::paint::coordinator::PaintInline::new(child).into_any_element()
}

#[cfg(test)]
pub(crate) fn paint_collect() -> (AnyElement, AnyElement) {
    let (open, close) = crate::paint::coordinator::PaintCollect::pair();
    (open.into_any_element(), close.into_any_element())
}
