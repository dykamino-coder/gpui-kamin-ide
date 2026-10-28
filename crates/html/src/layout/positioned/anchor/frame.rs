//! Сброс реестров одного кадра; между кадрами остаётся лишь выбранный вариант.

use super::AnchorState;

pub(crate) fn reset(f: &AnchorState) {
    f.named_seq.borrow_mut().clear();
    f.implicit.borrow_mut().clear();
    f.cb.borrow_mut().clear();
    f.cb_parent.borrow_mut().clear();
    f.out_of_flow.borrow_mut().clear();
    f.misses.take();
    let present = f.present.take();
    f.chosen.borrow_mut().retain(|key, _| present.contains(key));
    f.seq.set(0);
    f.tf_next.set(0);
    f.tf.borrow_mut().clear();
}

pub(crate) fn next_seq(anchors: &AnchorState) -> u32 {
    let s = &anchors.seq;
    let v = s.get() + 1;
    s.set(v);
    v
}

/// Run `f` with its own record of anchor misses (boxes whose anchor lookup
/// failed) and return them; the frame keeps every miss.
pub(crate) fn misses_of(anchors: &AnchorState, f: impl FnOnce()) -> std::collections::HashSet<u64> {
    let misses = &anchors.misses;
    let before = misses.take();
    f();
    let during = misses.replace(before);
    misses.borrow_mut().extend(during.iter().copied());
    during
}
