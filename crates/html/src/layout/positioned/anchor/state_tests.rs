//! Тесты: изоляция документов при чередовании операций кадра и prepaint якорей.

use super::{next_seq, reset, tf_map, tf_pop, tf_push, tf_under};
use crate::document::state::DocState;
use gpui::{Bounds, point, px, size};

#[test]
fn frame_keeps_only_last_success_for_boxes_seen_in_the_previous_frame() {
    use crate::style::computed::{Computed, Position};
    let doc = DocState::new(Default::default());
    let vcx = doc.values();
    let mut c = Computed {
        position: Some(Position::Absolute),
        anchor_key: 7,
        ..Default::default()
    };
    super::trials::Options::new(&c, &c, vcx).record(0, vcx);
    c.anchor_key = 8;
    super::trials::Options::new(&c, &c, vcx).record(0, vcx);
    doc.anchor_frame.present.borrow_mut().insert(7);
    doc.anchor_frame.out_of_flow.borrow_mut().insert(7, 1);
    reset(vcx);
    assert_eq!(doc.anchor_frame.chosen.borrow().len(), 1);
    assert!(doc.anchor_frame.chosen.borrow().contains_key(&7));
    assert!(doc.anchor_frame.out_of_flow.borrow().is_empty());
    reset(vcx);
    assert!(doc.anchor_frame.chosen.borrow().is_empty());
}

#[test]
fn frame_reset_does_not_consume_another_documents_geometry() {
    let a = DocState::new(Default::default());
    let b = DocState::new(Default::default());
    let av = a.frame_values();
    let bv = b.frame_values();
    let rect = Bounds::new(point(px(1.), px(2.)), size(px(10.), px(20.)));
    a.anchor_frame.cb.borrow_mut().insert(7, rect);
    b.anchor_frame.cb.borrow_mut().insert(7, rect);
    assert_eq!(next_seq(av), 1);
    assert_eq!(next_seq(bv), 1);
    assert_eq!(next_seq(av), 2);
    reset(av);
    assert_eq!(next_seq(av), 1);
    assert_eq!(next_seq(bv), 2);
    assert!(a.anchor_frame.cb.borrow().is_empty());
    assert_eq!(b.anchor_frame.cb.borrow().get(&7), Some(&rect));
    reset(bv);
    assert!(b.anchor_frame.cb.borrow().is_empty());
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

#[test]
fn deferred_context_restores_the_original_stack_without_changing_another_document() {
    let a = DocState::new(Default::default());
    let b = DocState::new(Default::default());
    let av = a.frame_values();
    let bv = b.frame_values();
    let rect = Bounds::new(point(px(2.), px(3.)), size(px(4.), px(5.)));
    tf_push([[1., 0., 10.], [0., 1., 20.]], av);
    let saved = super::TransformContext::capture(av.keep());
    tf_pop(av);
    tf_push([[1., 0., 30.], [0., 1., 40.]], av);
    tf_push([[1., 0., 50.], [0., 1., 60.]], bv);
    saved.enter(|| {
        assert_eq!(tf_map(rect, av).0.origin, point(px(12.), px(23.)));
        assert_eq!(tf_map(rect, bv).0.origin, point(px(52.), px(63.)));
    });
    assert_eq!(tf_map(rect, av).0.origin, point(px(32.), px(43.)));
    assert_eq!(tf_map(rect, bv).0.origin, point(px(52.), px(63.)));
}
