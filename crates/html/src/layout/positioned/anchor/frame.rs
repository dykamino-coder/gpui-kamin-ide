//! Кадр якорей: устаревание записей, запрос перестройки, сброс и порядковые номера.

use super::CB;
use crate::layout::positioned::anchor::AREA_LAST;
use crate::layout::positioned::anchor::AREA_NOW;
use crate::layout::positioned::anchor::CB_PARENT;
use crate::layout::positioned::anchor::CELL_LAST;
use crate::layout::positioned::anchor::CELL_NOW;
use crate::layout::positioned::anchor::IMPLICIT;
use crate::layout::positioned::anchor::LAST_IMPLICIT;
use crate::layout::positioned::anchor::LAST_NAMED;
use crate::layout::positioned::anchor::NAMED;
use crate::layout::positioned::anchor::NAMED_SEQ;
use crate::layout::positioned::anchor::REFRAMES;
use crate::layout::positioned::anchor::SEQ;
use crate::layout::positioned::anchor::TF_NEXT;
use crate::layout::positioned::anchor::USED_LAST;
use gpui::Window;

/// Sizes resolved before layout from the previous frame are only correct once
/// the anchor geometry they read has settled; nothing else repaints a static
/// page, so ask for one more frame while it is still changing.
pub(super) fn reframe_if_stale(window: &mut Window) {
    if REFRAMES.with(|r| r.get()) < 8 {
        REFRAMES.with(|r| r.set(r.get() + 1));
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
pub fn reset() {
    crate::layout::positioned::absolute_overflow::reset();
    NAMED.with(|m| m.borrow_mut().clear());
    let named = NAMED_SEQ.with(|v| std::mem::take(&mut *v.borrow_mut()));
    LAST_NAMED.with(|v| *v.borrow_mut() = named);
    let implicit = IMPLICIT.with(|m| std::mem::take(&mut *m.borrow_mut()));
    LAST_IMPLICIT.with(|m| *m.borrow_mut() = implicit);
    CB.with(|m| m.borrow_mut().clear());
    CB_PARENT.with(|m| m.borrow_mut().clear());
    let area = AREA_NOW.with(|m| std::mem::take(&mut *m.borrow_mut()));
    AREA_LAST.with(|m| *m.borrow_mut() = area);
    let cell = CELL_NOW.with(|m| std::mem::take(&mut *m.borrow_mut()));
    CELL_LAST.with(|m| *m.borrow_mut() = cell);
    SEQ.with(|s| s.set(0));
    TF_NEXT.with(|n| n.set(0));
    USED_LAST.with(|u| u.set(false));
}

/// Порядковый номер сборки элемента в кадре: зовёт `render::element` в
/// порядке дерева, номер устойчив от кадра к кадру (дерево то же).
pub fn next_seq() -> u32 {
    SEQ.with(|s| {
        let v = s.get() + 1;
        s.set(v);
        v
    })
}
