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

/// Номер последней сборки кадра; повтор блоков продвигает его сам (layout::reuse).
pub(crate) fn seq(anchors: &AnchorState) -> u32 {
    anchors.seq.get()
}

pub(crate) fn set_seq(anchors: &AnchorState, seq: u32) {
    anchors.seq.set(seq);
}

pub(crate) fn next_seq(anchors: &AnchorState) -> u32 {
    let s = &anchors.seq;
    let v = s.get() + 1;
    s.set(v);
    v
}

/// Run `f` with its own record of anchor misses (boxes whose anchor lookup
/// failed) and return them; the frame keeps every miss.
pub(crate) async fn misses_of_async(
    anchors: &AnchorState,
    f: impl AsyncFnOnce(),
) -> std::collections::HashSet<u64> {
    struct Misses<'a> {
        anchors: &'a AnchorState,
        before: std::collections::HashSet<u64>,
    }
    impl Drop for Misses<'_> {
        fn drop(&mut self) {
            let mut during = self.anchors.misses.take();
            during.extend(self.before.drain());
            self.anchors.misses.replace(during);
        }
    }
    let guard = Misses {
        anchors,
        before: anchors.misses.take(),
    };
    f().await;
    let during = anchors.misses.borrow().clone();
    drop(guard);
    during
}
