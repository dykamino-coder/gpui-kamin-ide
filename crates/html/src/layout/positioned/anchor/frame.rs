//! Кадр якорей: устаревание записей, запрос перестройки, сброс и порядковые номера.

use crate::document::state::ValueCx;
use gpui::Window;

/// Sizes resolved before layout from the previous frame are only correct once
/// the anchor geometry they read has settled; nothing else repaints a static
/// page, so ask for one more frame while it is still changing.
pub(super) fn reframe_if_stale(window: &mut Window, vcx: ValueCx<'_>) {
    if vcx.doc.anchor_frame.reframes.get() < 8 {
        vcx.doc
            .anchor_frame
            .reframes
            .set(vcx.doc.anchor_frame.reframes.get() + 1);
        request_rebuild(window);
    }
}

/// Rebuild the page on the next frame. Called from `prepaint`, where
/// `window.refresh()` is ignored (GPUI drops invalidation while drawing), so the
/// view is notified from a next-frame callback instead.
pub(super) fn request_rebuild(window: &mut Window) {
    if let Some(view) = window.current_view_opt() {
        window.on_next_frame(move |window, cx| {
            cx.notify(view);
            window.refresh();
        });
    }
}

/// Расходник кадра — чистится в `interact::frame_sanitize`. Реестры текущего
/// кадра не выбрасываются, а переезжают в `LAST_*`.
pub fn reset(vcx: ValueCx<'_>) {
    vcx.doc.anchor_frame.named.borrow_mut().clear();
    let named = std::mem::take(&mut *vcx.doc.anchor_frame.named_seq.borrow_mut());
    *vcx.doc.anchor_frame.last_named.borrow_mut() = named;
    let implicit = std::mem::take(&mut *vcx.doc.anchor_frame.implicit.borrow_mut());
    *vcx.doc.anchor_frame.last_implicit.borrow_mut() = implicit;
    vcx.doc.anchor_frame.cb.borrow_mut().clear();
    vcx.doc.anchor_frame.cb_parent.borrow_mut().clear();
    let area = std::mem::take(&mut *vcx.doc.anchor_frame.area_now.borrow_mut());
    *vcx.doc.anchor_frame.area_last.borrow_mut() = area;
    let cell = std::mem::take(&mut *vcx.doc.anchor_frame.cell_now.borrow_mut());
    *vcx.doc.anchor_frame.cell_last.borrow_mut() = cell;
    vcx.doc.anchor_frame.seq.set(0);
    vcx.doc.anchor_frame.tf_next.set(0);
    vcx.doc.anchor_frame.used_last.set(false);
}

/// Порядковый номер сборки элемента в кадре: зовёт `render::element` в
/// порядке дерева, номер устойчив от кадра к кадру (дерево то же).
pub fn next_seq(vcx: ValueCx<'_>) -> u32 {
    {
        let s = &vcx.doc.anchor_frame.seq;
        let v = s.get() + 1;
        s.set(v);
        v
    }
}
