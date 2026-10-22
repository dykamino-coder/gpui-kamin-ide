//! Размеры текущего кадра не зависят от сохранённой геометрии документа.

use super::{SizeInputs, resolve_current};
use crate::document::state::DocState;
use crate::layout::positioned::anchor::AnchorRec;
use crate::style::computed::{Computed, Position};
use crate::style::values::value::Len;
use gpui::{Bounds, point, px, size};

fn record(width: f32) -> AnchorRec {
    let rect = Bounds::new(point(px(0.), px(0.)), size(px(width), px(20.)));
    AnchorRec {
        rect,
        rect_tf: rect,
        clip: rect,
        id: 1,
        cb: 0,
        hidden: false,
        tf_top: 0,
    }
}

#[test]
fn anchor_size_reads_current_geometry_and_falls_back_without_history() {
    let doc = DocState::new(Default::default());
    let vcx = doc.frame_values();
    doc.anchor_frame
        .named_seq
        .borrow_mut()
        .push(("--a".into(), 1, record(40.)));
    let mut c = Computed {
        position: Some(Position::Absolute),
        anchor_seq: 2,
        width: Len::parse("anchor-size(--a width, 9px)", vcx),
        ..Default::default()
    };
    let original = c.clone();
    resolve_current(&mut c, &Computed::default(), SizeInputs::default(), vcx);
    assert_eq!(c.width, Some(Len::Px(40.)));
    doc.anchor_frame.named_seq.borrow_mut().clear();
    c = original;
    resolve_current(&mut c, &Computed::default(), SizeInputs::default(), vcx);
    assert_eq!(c.width, Some(Len::Px(9.)));
}
