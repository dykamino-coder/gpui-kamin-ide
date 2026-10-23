//! Материализованный стиль: тот же, что у стадии вычисления под тем же родителем; правка каскадного
//! стиля, другие унаследованные данные родителя и каскадное `inherit` его не используют.

use super::*;
use crate::dom::Element;
use crate::style::values::value::Len;

fn element<'a>(nodes: &'a [Node], tag: &str) -> &'a Element {
    nodes
        .iter()
        .find_map(|n| match n {
            Node::Element(e) if e.tag == tag => Some(e),
            Node::Element(e) => Some(element(&e.children, tag)),
            _ => None,
        })
        .unwrap()
}

fn generation(vcx: ValueCx<'_>) -> u64 {
    vcx.doc.metrics.generation()
}

fn slot_parent(own: &SharedStyle) -> Option<Rc<Computed>> {
    own.computed().0.borrow().as_ref().map(|m| m.parent.clone())
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
        .computed()
        .get(&p.style, &parent, generation(vcx))
        .unwrap();
    assert_eq!(computed.font_size, Some(Len::Px(40.0)));
    assert_eq!(computed.text_indent, Some(Len::Px(40.0)));
    assert_eq!(computed.color, parent.color);
    // The box style reuses it under a parent box with the same inherited data.
    let mut used_parent = (*parent).clone();
    used_parent.cb_height_def = !used_parent.cb_height_def;
    let used = box_style(&used_parent, &p.style, vcx);
    assert_eq!(used.font_size, Some(Len::Px(40.0)));
    assert!(body.style.computed().0.borrow().is_some());
}

#[test]
fn an_edit_or_another_parent_recomputes() {
    let parsed = crate::dom::parse(PAGE, "");
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let p = element(&parsed.nodes, "p");
    let parent = slot_parent(&p.style).unwrap();
    // A first-line layer or an animation frame changes the inherited font size.
    let mut bigger = (*parent).clone();
    bigger.font_size = Some(Len::Px(30.0));
    assert!(
        p.style
            .computed()
            .get(&p.style, &bigger, generation(vcx))
            .is_none()
    );
    assert_eq!(
        box_style(&bigger, &p.style, vcx).font_size,
        Some(Len::Px(60.0))
    );
    // A layout copy that edits the cascaded style drops its materialized style.
    let mut copy = p.clone();
    assert!(
        copy.style
            .computed()
            .get(&copy.style, &parent, generation(vcx))
            .is_some()
    );
    copy.style.text_indent = Some(Len::Px(5.0));
    assert!(
        copy.style
            .computed()
            .get(&copy.style, &parent, generation(vcx))
            .is_none()
    );
    assert!(
        p.style
            .computed()
            .get(&p.style, &parent, generation(vcx))
            .is_some()
    );
}

#[test]
fn cascaded_inherit_is_reused_only_under_the_same_parent() {
    let parsed = crate::dom::parse(PAGE, "");
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let span = element(&parsed.nodes, "span");
    let parent = slot_parent(&span.style).unwrap();
    assert!(
        span.style
            .computed()
            .get(&span.style, &parent, generation(vcx))
            .is_some()
    );
    let copy = (*parent).clone();
    assert!(
        span.style
            .computed()
            .get(&span.style, &copy, generation(vcx))
            .is_none()
    );
}
