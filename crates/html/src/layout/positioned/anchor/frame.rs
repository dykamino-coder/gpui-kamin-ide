//! Сброс реестров одного кадра; между кадрами остаётся лишь выбранный вариант.

use crate::document::state::ValueCx;

pub fn reset(vcx: ValueCx<'_>) {
    let f = &vcx.doc.anchor_frame;
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

pub fn next_seq(vcx: ValueCx<'_>) -> u32 {
    let s = &vcx.doc.anchor_frame.seq;
    let v = s.get() + 1;
    s.set(v);
    v
}
