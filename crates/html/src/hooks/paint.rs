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

pub(crate) use crate::paint::coordinator::LineCapture;

/// Paint a block and capture the paint it deferred past its own output.
pub(crate) async fn paint_capturing_async<R>(
    window: &mut Window,
    f: impl AsyncFnOnce(&mut Window) -> R,
) -> (R, std::rc::Rc<LineCapture>) {
    crate::paint::coordinator::capture_async(window, f).await
}

/// Replay a complete deferred paint record into this frame's queues.
pub(crate) fn replay_deferred(
    window: &mut Window,
    record: &LineCapture,
) -> std::rc::Rc<LineCapture> {
    crate::paint::coordinator::replay(window, record)
}

#[cfg(test)]
pub(crate) fn paint_reopen<R>(window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
    crate::paint::coordinator::reopen(window, f)
}

pub(crate) async fn paint_reopen_async<R>(
    window: &mut Window,
    f: impl AsyncFnOnce(&mut Window) -> R,
) -> R {
    crate::paint::coordinator::reopen_async(window, f).await
}

pub(crate) use crate::layout::fragment::paint_source::{
    BareRecords, Replay, SlotScopes, SourceSlices,
};

pub(crate) async fn prepaint_source_record_async(
    extent: gpui::Bounds<gpui::Pixels>,
    segments: std::rc::Rc<[gpui::PaintSegment]>,
    slices: SourceSlices,
    window: &mut Window,
    cx: &mut gpui::App,
    stack: &mut gpui::PaintStack,
    prepaint: impl AsyncFnOnce(&mut Window, &mut gpui::App, &mut gpui::PaintStack),
) -> Result<(), gpui::PaintRecordError> {
    crate::paint::coordinator::segments::prepaint_async(
        extent, segments, slices, window, cx, stack, prepaint,
    )
    .await
}

pub(crate) async fn paint_source_record_async(
    extent: gpui::Bounds<gpui::Pixels>,
    segments: std::rc::Rc<[gpui::PaintSegment]>,
    slices: SourceSlices,
    window: &mut Window,
    cx: &mut gpui::App,
    stack: &mut gpui::PaintStack,
    paint: impl AsyncFnOnce(&mut Window, &mut gpui::App, &mut gpui::PaintStack),
) -> Result<(gpui::PaintSegmentRecords, BareRecords), gpui::PaintRecordError> {
    crate::paint::coordinator::segments::record_async(
        extent, segments, slices, window, cx, stack, paint,
    )
    .await
}

pub(crate) async fn paint_source_node_async(
    id: u64,
    cb: Option<u64>,
    placed: gpui::Point<gpui::Pixels>,
    window: &mut Window,
    cx: &mut gpui::App,
    stack: &mut gpui::PaintStack,
    paint: impl AsyncFnOnce(&mut Window, &mut gpui::App, &mut gpui::PaintStack),
) {
    crate::paint::coordinator::segments::node_async(id, cb, placed, window, cx, stack, paint).await;
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

/// Hit scopes of the placements: a slot's copy scope, else the current scope
/// under the placement's per-fragment presentation.
pub(crate) fn presented_hit_scopes(
    slots: std::rc::Rc<[(gpui::PaintSegmentId, u32, gpui::PaintHitScope)]>,
    presented: &[(gpui::PaintSegmentId, u32, gpui::TransformationMatrix)],
    window: &Window,
) -> std::rc::Rc<[(gpui::PaintSegmentId, u32, gpui::PaintHitScope)]> {
    if presented.is_empty() {
        return slots;
    }
    let base = window.paint_hit_scope();
    let mut all = slots.to_vec();
    for &(segment, ordinal, matrix) in presented {
        if !all.iter().any(|(s, o, _)| *s == segment && *o == ordinal) {
            all.push((
                segment,
                ordinal,
                window.paint_hit_scope_under(&base, matrix),
            ));
        }
    }
    all.into()
}
