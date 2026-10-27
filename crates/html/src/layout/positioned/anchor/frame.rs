//! Сброс реестров одного кадра; между кадрами остаётся лишь выбранный вариант.

use super::AnchorState;

pub(crate) fn reset(f: &AnchorState) {
    f.named_seq.borrow_mut().clear();
    f.implicit.borrow_mut().clear();
    f.cb.borrow_mut().clear();
    f.cb_parent.borrow_mut().clear();
    f.out_of_flow.borrow_mut().clear();
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
