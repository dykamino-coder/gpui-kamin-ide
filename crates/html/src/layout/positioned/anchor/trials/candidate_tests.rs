//! Пробы compute/zoom не меняют исходный элемент и наследуют только реальный вариант.

use super::*;
use crate::dom::{Element, Node};
use crate::style::cascade::materialize::{box_style, materialize};
use crate::style::computed::Computed;
use crate::style::css::parse_decls;
use crate::style::values::value::Len;

fn element<'a>(nodes: &'a [Node], tag: &str) -> &'a Element {
    element_opt(nodes, tag).unwrap()
}

fn element_opt<'a>(nodes: &'a [Node], tag: &str) -> Option<&'a Element> {
    nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.tag == tag => Some(e),
        Node::Element(e) => element_opt(&e.children, tag),
        _ => None,
    })
}

#[test]
fn named_try_is_computed_once_and_descendants_see_the_computed_variant() {
    let parsed = crate::dom::parse(
        r#"<body><div style="position:absolute;zoom:2;width:5px;position-try-fallbacks:--fit"><b style="width:inherit">x</b></div></body>"#,
        "",
    );
    let vcx = parsed.values();
    vcx.doc.try_rules.borrow_mut().insert(
        "--fit".into(),
        parse_decls("width:10px;font-size:99px;color:red"),
    );
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let div = element(&parsed.nodes, "div");
    let child = element(&div.children, "b");
    let base = box_style(&BoxStyle::default(), &div.style, vcx);
    let options = super::super::Options::new(&base, &BoxStyle::default(), vcx, &parsed.doc.layout);
    let (_, trial) = &options.styles[1];
    assert_eq!(trial.width, Some(Len::Px(20.)));
    assert_eq!(trial.font_size, base.font_size);
    assert_eq!(trial.color, base.color);
    let mut layout_copy = trial.clone();
    layout_copy.width = Some(Len::Px(999.));
    layout_copy.font_size = Some(Len::Px(7.));
    assert_eq!(
        box_style(&layout_copy, &child.style, vcx).width,
        Some(Len::Px(20.))
    );
    assert_eq!(div.style.computed().width, Some(Len::Px(10.)));
    options.record(1, vcx);
    let next = super::super::Options::new(&base, &BoxStyle::default(), vcx, &parsed.doc.layout);
    assert_eq!(next.styles[0].0, 1);
    assert_eq!(
        box_style(&next.styles[0].1, &child.style, vcx).width,
        Some(Len::Px(20.))
    );
    assert_eq!(
        box_style(&base, &child.style, vcx).width,
        Some(Len::Px(10.))
    );
}

#[test]
fn remembered_geometry_and_computed_parent_share_only_accepted_values() {
    let parsed = crate::dom::parse(
        r#"<body><div style="position:absolute;font-size:10px;position-try-fallbacks:--fit">x</div></body>"#,
        "",
    );
    let vcx = parsed.values();
    vcx.doc
        .try_rules
        .borrow_mut()
        .insert("--fit".into(), parse_decls("width:2em"));
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let div = element(&parsed.nodes, "div");
    let parent = BoxStyle::default();
    let base = box_style(&parent, &div.style, vcx);
    let options = super::super::Options::new(&base, &parent, vcx, &parsed.doc.layout);
    assert_eq!(options.styles[1].1.width, Some(Len::Px(20.)));
    options.record(1, vcx);
    let mut changed = div.clone();
    changed.style.cascaded_mut().font_size = Some(Len::Px(30.));
    let nodes = [Node::Element(changed)];
    materialize(&nodes, &Rc::new(Computed::default()), vcx);
    let base = box_style(&parent, &element(&nodes, "div").style, vcx);
    let options = super::super::Options::new(&base, &parent, vcx, &parsed.doc.layout);
    assert_eq!(options.styles[0].0, 1);
    assert_eq!(options.styles[0].1.width, Some(Len::Px(20.)));
    let raw = &options.styles[0].1.element_style.as_ref().unwrap().computed;
    assert_eq!(raw.width, Some(Len::Px(20.)));
    assert_eq!(raw.font_size, Some(Len::Px(30.)));
    let fresh = build(&base, &parent, vcx);
    assert_eq!(fresh[1].1.width, Some(Len::Px(60.)));
}
