//! Document isolation across interleaved anchor frame and prepaint operations.

use super::{next_seq, reset, tf_map, tf_pop, tf_push, tf_under};
use crate::document::state::DocState;
use gpui::{Bounds, point, px, size};

#[test]
fn frame_reset_does_not_consume_another_documents_geometry() {
    let a = DocState::new(Default::default());
    let b = DocState::new(Default::default());
    let av = a.frame_values();
    let bv = b.frame_values();
    a.anchor_frame.area_now.borrow_mut().insert(7, (10.0, 20.0));
    b.anchor_frame.area_now.borrow_mut().insert(7, (30.0, 40.0));
    assert_eq!(next_seq(av), 1);
    assert_eq!(next_seq(bv), 1);
    assert_eq!(next_seq(av), 2);
    reset(av);
    assert_eq!(next_seq(av), 1);
    assert_eq!(next_seq(bv), 2);
    assert_eq!(
        a.anchor_frame.area_last.borrow().get(&7),
        Some(&(10.0, 20.0))
    );
    assert_eq!(
        b.anchor_frame.area_now.borrow().get(&7),
        Some(&(30.0, 40.0))
    );
    assert!(b.anchor_frame.area_last.borrow().is_empty());
    reset(bv);
    assert_eq!(
        b.anchor_frame.area_last.borrow().get(&7),
        Some(&(30.0, 40.0))
    );
}

#[test]
fn retained_context_selects_its_own_transform_stack() {
    let a = DocState::new(Default::default());
    let b = DocState::new(Default::default());
    let retained = a.frame_values().keep();
    let av = retained.vcx();
    let bv = b.frame_values();
    let rect = Bounds::new(point(px(2.0), px(3.0)), size(px(4.0), px(5.0)));
    tf_push([[1.0, 0.0, 10.0], [0.0, 1.0, 20.0]], av);
    assert!(tf_under(1, av));
    assert!(!tf_under(1, bv));
    assert_eq!(tf_map(rect, bv), (rect, 0));
    tf_push([[1.0, 0.0, 30.0], [0.0, 1.0, 40.0]], bv);
    assert_eq!(tf_map(rect, av).0.origin, point(px(12.0), px(23.0)));
    assert_eq!(tf_map(rect, bv).0.origin, point(px(32.0), px(43.0)));
    tf_pop(av);
    assert_eq!(tf_map(rect, av), (rect, 0));
    assert!(tf_under(1, bv));
    tf_pop(bv);
}
