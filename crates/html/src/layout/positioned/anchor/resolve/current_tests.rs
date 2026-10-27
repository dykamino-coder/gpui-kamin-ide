//! Размеры текущего кадра не зависят от сохранённой геометрии документа.

use super::{SizeInputs, resolve_current};
use crate::document::state::DocState;
use crate::layout::positioned::anchor::AnchorRec;
use crate::style::computed::{BoxStyle, Computed, Position, PositionAnchor};
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
    doc.layout
        .anchors
        .named_seq
        .borrow_mut()
        .push(("--a".into(), 1, record(40.)));
    let mut c = BoxStyle::new(Computed {
        position: Some(Position::Absolute),
        width: Len::parse("anchor-size(--a width, 9px)", vcx),
        ..Default::default()
    });
    c.anchor_seq = 2;
    let original = c.clone();
    resolve_current(
        &mut c,
        &BoxStyle::default(),
        SizeInputs::default(),
        vcx,
        &doc.layout,
    );
    assert_eq!(c.width, Some(Len::Px(40.)));
    doc.layout.anchors.named_seq.borrow_mut().clear();
    c = original;
    resolve_current(
        &mut c,
        &BoxStyle::default(),
        SizeInputs::default(),
        vcx,
        &doc.layout,
    );
    assert_eq!(c.width, Some(Len::Px(9.)));
}

#[test]
fn absent_default_anchor_keeps_explicit_anchor_inset_stretch_sizing() {
    let doc = DocState::new(Default::default());
    let vcx = doc.frame_values();
    for position_anchor in [None, Some(PositionAnchor::Named("--missing".into()))] {
        let mut c = BoxStyle::new(Computed {
            position: Some(Position::Absolute),
            position_anchor,
            position_area: crate::style::values::position_area::parse_area("span-bottom right"),
            ..Default::default()
        });
        c.inset.top = Len::parse("anchor(--explicit bottom)", vcx);
        c.inset.bottom = Some(Len::Px(0.));
        resolve_current(
            &mut c,
            &BoxStyle::default(),
            SizeInputs {
                area: Some((100., 67.)),
                cell: None,
                ..Default::default()
            },
            vcx,
            &doc.layout,
        );
        assert_eq!(c.height, Some(Len::Px(67.)));
    }
}

#[test]
fn trial_computed_parent_is_local_to_the_matching_frame_root() {
    use crate::layout::cx::LayoutCx;
    use crate::style::computed::ElementStyle;
    use std::rc::Rc;

    let original = Rc::new(Computed::default());
    let mut trial = (*original).clone();
    trial.width = Some(Len::Px(20.));
    let variant = Rc::new(trial);
    let mut cx = LayoutCx::default();
    cx.position.oof_computed = Some((
        7,
        ElementStyle {
            original: original.clone(),
            computed: variant.clone(),
            parent: None,
        },
    ));
    let mut root = BoxStyle::shared(original.clone());
    root.anchor_key = 7;
    root.width = Some(Len::Px(999.));
    super::prepare(&mut root, &BoxStyle::default(), &cx);
    assert!(Rc::ptr_eq(
        &root.element_style.as_ref().unwrap().computed,
        &variant
    ));
    assert_eq!(root.width, Some(Len::Px(999.)));
    let mut other = BoxStyle::shared(original);
    other.anchor_key = 8;
    super::prepare(&mut other, &root, &cx);
    assert!(other.element_style.is_none());
    assert!(LayoutCx::default().position.oof_computed.is_none());
}
