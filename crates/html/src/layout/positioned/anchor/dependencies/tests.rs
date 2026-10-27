//! Зависимости проб и циклическая приемлемость содержащих блоков.

use super::Dependencies;
use crate::document::state::DocState;
use crate::layout::positioned::anchor::AnchorRec;
use crate::style::computed::{BoxStyle, Computed, Position};
use crate::style::css::parse_decls;
use crate::style::values::{position_area::parse_try_fallbacks, value::Len};
use gpui::{Bounds, point, px, size};

fn record(id: u64, cb: u64) -> AnchorRec {
    let rect = Bounds::new(point(px(0.), px(0.)), size(px(30.), px(20.)));
    AnchorRec {
        rect,
        rect_tf: rect,
        clip: rect,
        id,
        cb,
        hidden: false,
        tf_top: 0,
    }
}

#[test]
fn waits_for_later_inflow_anchor_and_all_candidate_dependencies() {
    let doc = DocState::new(Default::default());
    let vcx = doc.values();
    let mut c = BoxStyle::new(Computed {
        position: Some(Position::Absolute),
        width: Len::parse("anchor-size(--base width, 1px)", vcx),
        position_try_fallbacks: parse_try_fallbacks("--other"),
        ..Default::default()
    });
    c.anchor_key = 3;
    c.anchor_seq = 3;
    doc.try_rules.borrow_mut().insert(
        "--other".into(),
        parse_decls("width: anchor-size(--other width, 2px)"),
    );
    let deps = Dependencies::of(&c, &BoxStyle::default(), vcx, &doc.layout);
    assert!(!deps.ready());
    doc.layout
        .anchors
        .named_seq
        .borrow_mut()
        .push(("--base".into(), 4, record(4, 0)));
    assert!(!deps.ready());
    doc.layout
        .anchors
        .named_seq
        .borrow_mut()
        .push(("--other".into(), 5, record(5, 0)));
    assert!(deps.ready());
    c.position_try_fallbacks.clear();
    assert!(Dependencies::of(&c, &BoxStyle::default(), vcx, &doc.layout).ready());
    doc.layout.anchors.named_seq.borrow_mut().clear();
    assert!(!deps.ready());
}

#[test]
fn later_oof_or_own_descendant_does_not_break_dependency_cycle() {
    let doc = DocState::new(Default::default());
    let vcx = doc.values();
    let mut c = BoxStyle::new(Computed {
        position: Some(Position::Absolute),
        width: Len::parse("anchor-size(--a width, 1px)", vcx),
        ..Default::default()
    });
    c.anchor_key = 3;
    c.anchor_seq = 3;
    let deps = Dependencies::of(&c, &BoxStyle::default(), vcx, &doc.layout);
    doc.layout
        .anchors
        .named_seq
        .borrow_mut()
        .push(("--a".into(), 4, record(4, 0)));
    doc.layout.anchors.out_of_flow.borrow_mut().insert(4, 4);
    assert!(!deps.ready());
    doc.layout.anchors.out_of_flow.borrow_mut().clear();
    doc.layout.anchors.named_seq.borrow_mut()[0].2.cb = 3;
    assert!(!deps.ready());
    doc.layout.anchors.named_seq.borrow_mut()[0].2.cb = 7;
    doc.layout.anchors.cb_parent.borrow_mut().insert(7, 8);
    doc.layout.anchors.cb_parent.borrow_mut().insert(8, 7);
    assert!(!deps.ready());
    doc.layout.anchors.cb_parent.borrow_mut().insert(8, 0);
    assert!(deps.ready());
    for cb in 8..108 {
        doc.layout.anchors.cb_parent.borrow_mut().insert(cb, cb + 1);
    }
    doc.layout.anchors.cb_parent.borrow_mut().insert(108, 0);
    assert!(deps.ready());
}
