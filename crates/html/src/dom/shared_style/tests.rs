//! Проверки стадий и транспорта стиля элемента.

use super::*;
use crate::style::values::value::Len;

#[test]
fn layout_edits_do_not_change_a_shared_cascade() {
    let parsed = crate::dom::parse(r#"<div style="width: 50%">a</div>"#, "");
    let vcx = parsed.values();
    crate::style::cascade::materialize::materialize(
        &parsed.nodes,
        &Rc::new(Computed::default()),
        vcx,
    );
    let original = &crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == "div")
        .unwrap()
        .style;
    let mut copy = original.clone();
    assert!(Rc::ptr_eq(&original.cascaded, &copy.cascaded));
    copy.used_mut().width = Some(Len::Px(100.0));
    assert_eq!(original.used().width, Some(Len::Pct(0.5)));
    assert_eq!(copy.used().width, Some(Len::Px(100.0)));
    // Вычисленная стадия хранит материализованный стиль без used-значения.
    assert_eq!(copy.computed().width, Some(Len::Pct(0.5)));
    // Каскадный и материализованный вычисленный стили не изменились.
    assert!(Rc::ptr_eq(&original.cascaded, &copy.cascaded));
    assert!(Rc::ptr_eq(
        &original.slot().any().unwrap(),
        &copy.slot().any().unwrap()
    ));
    assert_eq!(copy.cascaded().width, Some(Len::Pct(0.5)));
}

#[test]
fn a_style_without_materialization_has_one_stage() {
    let mut style = SharedStyle::from(Computed::default());
    style.used_mut().width = Some(Len::Px(10.0));
    assert_eq!(style.cascaded().width, Some(Len::Px(10.0)));
    assert!(style.used.is_none());
}

#[test]
fn element_copies_share_pseudo_layers_and_keyframes() {
    let mut node = crate::dom::table_boxes::anon_element("div", vec![]);
    node.hover = Some(Computed::default().into());
    node.first_letter = node.hover.clone();
    node.first_line = node.hover.clone();
    node.anim = Some(Rc::new(vec![(0.0, Computed::default())]));
    node.style.cascaded_mut().width = Some(Len::Pct(0.5));
    let mut copy = node.clone();
    copy.style.used_mut().width = Some(Len::Px(100.0));
    assert_eq!(node.style.used().width, Some(Len::Pct(0.5)));
    assert!(Rc::ptr_eq(
        node.anim.as_ref().unwrap(),
        copy.anim.as_ref().unwrap()
    ));
    copy.hover.as_mut().unwrap().cascaded_mut().font_size = Some(Len::Px(32.0));
    assert!(node.hover.as_ref().unwrap().cascaded().font_size.is_none());
    assert!(
        copy.first_letter
            .as_ref()
            .unwrap()
            .cascaded()
            .font_size
            .is_none()
    );
    assert!(std::mem::size_of::<crate::dom::Element>() < 512);
}
