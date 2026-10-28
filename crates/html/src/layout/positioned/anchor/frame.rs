//! Сброс реестров одного кадра; между кадрами остаётся лишь выбранный вариант.

use crate::document::state::ValueCx;

pub fn reset(vcx: ValueCx<'_>) {
    let f = &vcx.doc.anchor_frame;
    f.named_seq.borrow_mut().clear();
    f.implicit.borrow_mut().clear();
    f.cb.borrow_mut().clear();
    f.cb_parent.borrow_mut().clear();
    f.out_of_flow.borrow_mut().clear();
    f.misses.borrow_mut().clear();
    let present = f.present.take();
    f.chosen.borrow_mut().retain(|key, _| present.contains(key));
    f.seq.set(0);
    f.tf_next.set(0);
    f.tf.borrow_mut().clear();
}

pub fn next_seq(vcx: ValueCx<'_>) -> u32 {
    let s = &vcx.doc.anchor_frame.seq;
    let v = s.get() + 1;
    s.set(v);
    v
}

/// Run `f` with its own record of anchor misses (boxes whose anchor lookup
/// failed) and return them; the frame keeps every miss.
pub(crate) fn misses_of(vcx: ValueCx<'_>, f: impl FnOnce()) -> std::collections::HashSet<u64> {
    let misses = &vcx.doc.anchor_frame.misses;
    let before = std::mem::take(&mut *misses.borrow_mut());
    f();
    let during = std::mem::replace(&mut *misses.borrow_mut(), before);
    misses.borrow_mut().extend(during.iter().copied());
    during
}
