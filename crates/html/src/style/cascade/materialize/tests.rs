//! Материализованный стиль: тот же, что у стадии вычисления под тем же родителем; правка каскадного
//! стиля, другие унаследованные данные родителя и каскадное `inherit` его не используют.

use super::*;
use crate::dom::Element;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;

pub(super) fn element<'a>(nodes: &'a [Node], tag: &str) -> &'a Element {
    find(nodes, tag).expect("element in document")
}

/// The first element `tag` in document order, at any depth.
pub(super) fn find<'a>(nodes: &'a [Node], tag: &str) -> Option<&'a Element> {
    nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.tag == tag => Some(e),
        Node::Element(e) => find(&e.children, tag),
        _ => None,
    })
}

fn generation(vcx: ValueCx<'_>) -> u64 {
    vcx.doc.metrics.generation()
}

fn slot_parent(own: &SharedStyle) -> Option<Rc<Computed>> {
    own.slot().0.borrow().as_ref().map(|m| m.parent.clone())
}

const PAGE: &str = r#"<style>
    body { font: 20px serif; color: rgb(0, 128, 0) }
    p { font-size: 2em; text-indent: 1em }
    span { padding-left: inherit }
</style><body><p>a <span>b</span></p></body>"#;

#[test]
fn materialized_style_is_the_computed_value_stage() {
    let parsed = crate::dom::parse(PAGE, "");
    let vcx = parsed.values();
    let root = Rc::new(Computed::default());
    materialize(&parsed.nodes, &root, vcx);
    let body = element(&parsed.nodes, "body");
    let p = element(&parsed.nodes, "p");
    let parent = slot_parent(&p.style).expect("p is materialized");
    assert_eq!(parent.font_size, Some(Len::Px(20.0)));
    let computed = p
        .style
        .slot()
        .get(p.style.cascaded(), &parent, generation(vcx))
        .unwrap();
    assert_eq!(computed.font_size, Some(Len::Px(40.0)));
    assert_eq!(computed.text_indent, Some(Len::Px(40.0)));
    assert_eq!(computed.color, parent.color);
    // The box style reuses it under a parent box with the same inherited data.
    let mut used_parent = BoxStyle::new((*parent).clone());
    used_parent.cb_height_def = !used_parent.cb_height_def;
    let used = box_style(&used_parent, &p.style, vcx);
    assert_eq!(used.font_size, Some(Len::Px(40.0)));
    assert!(body.style.slot().0.borrow().is_some());
}

#[test]
fn only_a_cascade_edit_invalidates_the_element_style() {
    let parsed = crate::dom::parse(PAGE, "");
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let p = element(&parsed.nodes, "p");
    let parent = slot_parent(&p.style).unwrap();
    // Размер шрифта коробки не является входом наследования элемента.
    let mut bigger = (*parent).clone();
    bigger.font_size = Some(Len::Px(30.0));
    assert!(
        p.style
            .slot()
            .get(p.style.cascaded(), &bigger, generation(vcx))
            .is_none()
    );
    assert_eq!(
        box_style(&BoxStyle::from(bigger.clone()), &p.style, vcx).font_size,
        Some(Len::Px(40.0))
    );
    // A layout copy that edits the cascaded style drops its materialized style.
    let mut copy = p.clone();
    assert!(
        copy.style
            .slot()
            .get(copy.style.cascaded(), &parent, generation(vcx))
            .is_some()
    );
    copy.style.cascaded_mut().text_indent = Some(Len::Px(5.0));
    assert!(
        copy.style
            .slot()
            .get(copy.style.cascaded(), &parent, generation(vcx))
            .is_none()
    );
    assert!(
        p.style
            .slot()
            .get(p.style.cascaded(), &parent, generation(vcx))
            .is_some()
    );
}

#[test]
fn used_values_lie_over_the_shared_computed_style() {
    let parsed = crate::dom::parse(PAGE, "");
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let p = element(&parsed.nodes, "p");
    let parent = slot_parent(&p.style).unwrap();
    let mut copy = p.clone();
    copy.style.used_mut().margin.top = Some(Len::Px(7.0));
    // The materialization stays valid and shared; the box takes the used value.
    assert!(
        copy.style
            .slot()
            .get(copy.style.cascaded(), &parent, generation(vcx))
            .is_some()
    );
    let used = box_style(
        &BoxStyle::shared(std::rc::Rc::clone(&parent)),
        &copy.style,
        vcx,
    );
    assert_eq!(used.margin.top, Some(Len::Px(7.0)));
    assert_eq!(used.text_indent, Some(Len::Px(40.0)));
    // Used-правка сохраняется и под другой коробкой без смены computed-стадии.
    let mut bigger = (*parent).clone();
    bigger.font_size = Some(Len::Px(30.0));
    let other = box_style(&BoxStyle::from(bigger.clone()), &copy.style, vcx);
    assert_eq!(other.margin.top, Some(Len::Px(7.0)));
    assert_eq!(other.text_indent, Some(Len::Px(40.0)));
    assert_ne!(p.style.used().margin.top, Some(Len::Px(7.0)));
}

#[test]
fn cascaded_inherit_takes_the_parent_element_value() {
    let parsed = crate::dom::parse(PAGE, "");
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let span = element(&parsed.nodes, "span");
    let parent = slot_parent(&span.style).unwrap();
    assert!(
        span.style
            .slot()
            .get(span.style.cascaded(), &parent, generation(vcx))
            .is_some()
    );
    // `inherit` depends on the parent's non-inherited values too: a parent with
    // other box values (a split inline, a hoisted block's new parent) is no hit,
    // and the box still takes the element parent's value.
    let mut sibling_box = (*parent).clone();
    sibling_box.padding.left = Some(Len::Px(99.0));
    assert!(
        span.style
            .slot()
            .get(span.style.cascaded(), &sibling_box, generation(vcx))
            .is_none()
    );
    assert_eq!(
        box_style(&BoxStyle::from(sibling_box.clone()), &span.style, vcx)
            .padding
            .left,
        parent.padding.left
    );
    // Все наследуемые значения берутся от элемента, включая неявное наследование.
    sibling_box.font_size = Some(Len::Px(30.0));
    assert_eq!(
        box_style(&BoxStyle::from(sibling_box.clone()), &span.style, vcx).font_size,
        parent.font_size
    );
    assert!(
        span.style
            .slot()
            .get(span.style.cascaded(), &sibling_box, generation(vcx))
            .is_none()
    );
    assert_eq!(
        box_style(&BoxStyle::from(sibling_box.clone()), &span.style, vcx)
            .padding
            .left,
        parent.padding.left
    );
}

#[test]
fn inherited_values_are_one_shared_group() {
    let parsed = crate::dom::parse(
        r#"<style>body { font: 20px serif } p { margin: 3px }</style><body><p>a <span>b</span></p></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let p = element(&parsed.nodes, "p");
    let span = element(&parsed.nodes, "span");
    let p_style = p.style.slot().any().unwrap();
    let span_style = span.style.slot().any().unwrap();
    // No inherited declaration of its own: the child shares its parent's group.
    assert!(Rc::ptr_eq(&p_style.inh.0, &span_style.inh.0));
    // A copy of the style (a box style) shares it too, until an inherited value changes.
    let mut copy = (*span_style).clone();
    assert!(Rc::ptr_eq(&copy.inh.0, &span_style.inh.0));
    copy.margin.top = Some(Len::Px(1.0));
    assert!(Rc::ptr_eq(&copy.inh.0, &span_style.inh.0));
    copy.font_size = Some(Len::Px(9.0));
    assert!(!Rc::ptr_eq(&copy.inh.0, &span_style.inh.0));
    assert_eq!(span_style.font_size, Some(Len::Px(20.0)));
}
