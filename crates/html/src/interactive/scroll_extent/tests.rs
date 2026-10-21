//! Тесты: колбэки extent сохраняют свой scrollport, даже когда другое дерево начинает фазу.
use super::*;
use crate::layout::table::anon::anon_element;
use gpui::point;

#[test]
fn interleaved_scroll_frames_reset_only_their_own_phase_buffers() {
    let mut root = anon_element("div", vec![]);
    root.node_id = 7;
    let outer = Frame::default();
    let outer = outer.begin(Some(&root)).expect("tree has an extent owner");
    let nested = Frame::default();
    let nested = nested.begin(Some(&root)).expect("independent tree owner");
    let a = Bounds::new(point(px(10.0), px(20.0)), size(px(30.0), px(40.0)));
    let b = Bounds::new(point(px(50.0), px(60.0)), size(px(70.0), px(80.0)));
    record_text(&outer, 8, a, None);
    record_text(&nested, 8, b, None);
    assert_eq!(outer.text_boxes.borrow()[&8], [10.0, 20.0, 40.0, 60.0]);
    assert_eq!(nested.text_boxes.borrow()[&8], [50.0, 60.0, 120.0, 140.0]);
    nested.begin(Some(&root));
    assert!(nested.text_boxes.borrow().is_empty());
    assert!(outer.text_boxes.borrow().contains_key(&8));
    assert!(Frame::parent_for(Some(&outer), 7).is_none());
    let captured = Frame::parent_for(Some(&outer), 9).expect("nested scrollport parent");
    captured.record_border_box(9, &Computed::default(), b, None);
    assert!(outer.boxes.borrow().contains_key(&9));
    assert!(nested.boxes.borrow().is_empty());
}
