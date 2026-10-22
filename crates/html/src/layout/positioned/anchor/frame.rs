//! Кадр якорей: устаревание записей, запрос перестройки, сброс и порядковые номера.

use crate::document::state::ValueCx;
use gpui::Window;

/// Extra frames one chain of unsettled frames may request.
pub(super) const REFRAME_BUDGET: u32 = 8;

/// Sizes resolved before layout from the previous frame are only correct once
/// the anchor geometry they read has settled; nothing else repaints a static
/// page, so ask for one more frame while it is still changing.
///
/// The budget counts frames, not stale records: every stale record of one
/// frame shares a single request, and a frame that checked its records and
/// found none stale ends the chain (`reset`), so a later geometry change
/// (resize, scroll) gets a fresh budget.
pub(super) fn reframe_if_stale(window: &mut Window, vcx: ValueCx<'_>) {
    if take_reframe(vcx) {
        request_rebuild(window);
    }
}

/// Budget step of `reframe_if_stale`: marks the frame stale and says whether
/// this call is the one that requests the extra frame.
pub(super) fn take_reframe(vcx: ValueCx<'_>) -> bool {
    let f = &vcx.doc.anchor_frame;
    f.frame_checked.set(true);
    f.frame_stale.set(true);
    if f.frame_requested.get() || f.reframes.get() >= REFRAME_BUDGET {
        return false;
    }
    f.frame_requested.set(true);
    f.reframes.set(f.reframes.get() + 1);
    true
}

/// Record that this frame compared its previous-frame anchor values, stale
/// or not: a checked frame without stale values ends the reframe chain.
pub(super) fn note_checked(vcx: ValueCx<'_>) {
    vcx.doc.anchor_frame.frame_checked.set(true);
}

/// Frame boundary for the reframe budget: a settled frame (checked, nothing
/// stale) restores the budget; an unchecked one (another tree build of the
/// same frame, or a frame that read no previous-frame values) keeps it.
fn end_reframe_frame(vcx: ValueCx<'_>) {
    let f = &vcx.doc.anchor_frame;
    if f.frame_checked.get() && !f.frame_stale.get() {
        f.reframes.set(0);
    }
    f.frame_checked.set(false);
    f.frame_stale.set(false);
    f.frame_requested.set(false);
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

/// Расходник кадра — чистится в `interactive::frame::frame_sanitize`. Реестры текущего
/// кадра не выбрасываются, а переезжают в `LAST_*`.
pub fn reset(vcx: ValueCx<'_>) {
    end_reframe_frame(vcx);
    vcx.doc.anchor_frame.named.borrow_mut().clear();
    let named = std::mem::take(&mut *vcx.doc.anchor_frame.named_seq.borrow_mut());
    *vcx.doc.anchor_frame.last_named.borrow_mut() = named;
    let implicit = std::mem::take(&mut *vcx.doc.anchor_frame.implicit.borrow_mut());
    *vcx.doc.anchor_frame.last_implicit.borrow_mut() = implicit;
    vcx.doc.anchor_frame.cb.borrow_mut().clear();
    vcx.doc.anchor_frame.cb_parent.borrow_mut().clear();
    let area = std::mem::take(&mut *vcx.doc.anchor_frame.area_now.borrow_mut());
    vcx.doc
        .anchor_frame
        .chosen
        .borrow_mut()
        .retain(|key, _| area.contains_key(key));
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

#[cfg(test)]
mod tests {
    use super::{REFRAME_BUDGET, note_checked, reset, take_reframe};
    use crate::document::state::DocState;

    #[test]
    fn box_absence_discards_last_successful_option() {
        let doc = DocState::new(Default::default());
        doc.anchor_frame
            .chosen
            .borrow_mut()
            .insert(17, super::super::trials::Chosen::legacy(2));
        reset(doc.frame_values());
        assert!(doc.anchor_frame.chosen.borrow().is_empty());
    }

    #[test]
    fn stale_records_of_one_frame_share_one_budget_step() {
        let doc = DocState::new(Default::default());
        let v = doc.frame_values();
        assert!(take_reframe(v));
        for _ in 0..20 {
            assert!(!take_reframe(v));
        }
        assert_eq!(doc.anchor_frame.reframes.get(), 1);
    }

    #[test]
    fn budget_bounds_a_chain_of_unsettled_frames() {
        let doc = DocState::new(Default::default());
        let v = doc.frame_values();
        let mut granted = 0;
        for _ in 0..3 * REFRAME_BUDGET {
            reset(v);
            granted += u32::from(take_reframe(v));
        }
        assert_eq!(granted, REFRAME_BUDGET);
    }

    #[test]
    fn settled_frame_restores_the_budget() {
        let doc = DocState::new(Default::default());
        let v = doc.frame_values();
        for _ in 0..REFRAME_BUDGET {
            reset(v);
            assert!(take_reframe(v));
        }
        reset(v);
        assert!(!take_reframe(v));
        // A frame without anchors read from the previous frame (or a second
        // tree build of the same frame) does not end the chain.
        reset(v);
        reset(v);
        assert!(!take_reframe(v));
        // A checked frame with nothing stale does: a later change (resize)
        // is followed again.
        reset(v);
        note_checked(v);
        reset(v);
        assert!(take_reframe(v));
    }
}
