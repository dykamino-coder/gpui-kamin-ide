//! Tests of the value comparison of box trees (`same_tree`, `same_layout_tree_as`).

use super::*;
use crate::dom::{Marks, Text};
use crate::style::computed::Computed;

fn text(s: &str) -> Node {
    Node::Text(Text::from_source(s.into(), None))
}

fn block(children: Vec<Node>) -> Element {
    let mut e = Element::synthetic("div", Computed::default(), Marks::default(), children);
    e.node_id = 7;
    e
}

#[test]
fn copies_with_equal_values_are_the_same_tree() {
    let a = block(vec![text("one"), Node::Element(block(vec![text("two")]))]);
    assert!(same_tree(&a, &a.clone()));
    let rebuilt = block(vec![text("one"), Node::Element(block(vec![text("two")]))]);
    assert!(same_tree(&a, &rebuilt));
}

#[test]
fn a_shared_shell_compares_like_its_box() {
    let a = block(vec![text("one")]);
    let mut shell = a.clone();
    shell.node_id = 0;
    let styles = BoxStyles::of(&a);
    assert!(same_tree_as(&shell, &styles, &a.children, &a));
    let onf = block(vec![text("onf")]);
    assert!(!same_tree_as(&shell, &styles, &onf.children, &a));
    let mut other = a.clone();
    other.tag = "p".into();
    assert!(!same_tree_as(&other, &styles, &a.children, &a));
    let mut restyled = a.clone();
    restyled.style = Computed {
        decor_box: 1,
        ..Computed::default()
    }
    .into();
    assert!(same_shell(&shell, &restyled), "styles are not the shell's");
    assert!(!same_tree_as(
        &shell,
        &BoxStyles::of(&restyled),
        &a.children,
        &a
    ));
    let layered = Element {
        hover: Some(Computed::default().into()),
        ..a.clone()
    };
    assert!(!same_tree_as(
        &shell,
        &BoxStyles::of(&layered),
        &a.children,
        &a
    ));
    assert!(same_tree_as(
        &shell,
        &BoxStyles::of(&layered),
        &a.children,
        &layered
    ));
    assert!(same_tree(
        &BoxStyles::of(&layered).apply(&layered),
        &layered
    ));
}

#[test]
fn a_changed_text_or_attribute_differs() {
    let a = block(vec![text("one")]);
    assert!(!same_tree(&a, &block(vec![text("onf")])));
    let mut b = a.clone();
    b.attrs.push(("class".into(), "x".into()));
    assert!(!same_tree(&a, &b));
    assert!(!same_tree(&a, &block(vec![text("one"), text("")])));
}

fn recoloured(e: &Element) -> Element {
    let mut style = Computed::default();
    style.inh.ui.color = Some(crate::style::values::value::Color {
        r: 0.5,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    });
    Element {
        style: style.into(),
        ..e.clone()
    }
}

#[test]
fn paint_only_values_keep_the_layout_of_a_subtree() {
    let inner = block(vec![text("two")]);
    let a = block(vec![text("one"), Node::Element(inner.clone())]);
    let b = block(vec![text("one"), Node::Element(recoloured(&inner))]);
    let mut shell = a.clone();
    shell.node_id = 0;
    let styles = BoxStyles::of(&a);
    assert!(!same_tree_as(&shell, &styles, &a.children, &b));
    assert!(same_layout_tree_as(&shell, &styles, &a.children, &b));
    let onf = block(vec![text("one"), Node::Element(block(vec![text("twp")]))]);
    assert!(
        !same_layout_tree_as(&shell, &styles, &a.children, &onf),
        "texts stay exact"
    );
    let mut classed = b;
    classed.attrs.push(("class".into(), "x".into()));
    assert!(
        !same_layout_tree_as(&shell, &styles, &a.children, &classed),
        "attributes stay exact"
    );
}

#[test]
fn a_root_box_ignores_paint_only_values_class_and_data_attributes() {
    let a = block(vec![]);
    let mut b = recoloured(&a);
    b.attrs.push(("class".into(), "alt".into()));
    b.attrs.push(("data-k".into(), "1".into()));
    assert!(!same_box(&a, &b));
    assert!(same_layout_box(&a, &b));
    let mut c = b.clone();
    c.attrs.push(("id".into(), "x".into()));
    assert!(!same_layout_box(&a, &c), "other attributes stay exact");
    let mut d = b.clone();
    d.node_id += 1;
    assert!(!same_layout_box(&a, &d));
}
