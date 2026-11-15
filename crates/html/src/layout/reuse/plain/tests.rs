//! Тесты инертного поддерева: обычный поток, однократный вердикт и флоаты в корне контекста.

use super::*;
use crate::dom::{Marks, Text};
use crate::style::computed::Computed;

fn el(tag: &str, children: Vec<Node>) -> Element {
    Element::synthetic(tag, Computed::default(), Marks::default(), children)
}

#[test]
fn text_blocks_are_plain_and_replaced_or_positioned_descendants_are_not() {
    let text = Node::Text(Text::from_source("a".into(), None));
    assert!(plain(&el(
        "p",
        vec![text.clone(), Node::Element(el("b", vec![]))]
    )));
    assert!(!plain(&el("div", vec![Node::Element(el("img", vec![]))])));
    let mut moved = el("span", vec![text]);
    moved.style.used_mut().reset_positioned.position = Some(Position::Absolute);
    assert!(!plain(&el("div", vec![Node::Element(moved.clone())])));
    assert!(!escaping_floats(&el("div", vec![Node::Element(moved)])));
    let mut floated = el("span", vec![]);
    floated.style.used_mut().reset_box_layout.float = Some(1);
    assert!(escaping_floats(&el("div", vec![Node::Element(floated)])));
}

#[test]
fn inert_verdict_is_computed_once_per_child_build() {
    let inert = Inert::default();
    assert!(inert.of(&el("p", vec![])));
    // The second question is answered from the first walk, not a new one.
    assert!(inert.of(&el("img", vec![])));
    assert!(!Inert::default().of(&el("img", vec![])));
}

#[test]
fn floats_inside_a_formatting_context_root_do_not_escape_the_block() {
    let floated = || {
        let mut f = el("span", vec![]);
        f.style.used_mut().reset_box_layout.float = Some(1);
        Node::Element(f)
    };
    // A float in a plain descendant is outside every root but the flow's.
    let plain_parent = el("div", vec![Node::Element(el("p", vec![floated()]))]);
    assert!(escaping_floats(&plain_parent));
    // Multicol and flow-root contain their floats (CSS 2.1 §9.4.1).
    let mut columns = el("div", vec![Node::Element(el("p", vec![floated()]))]);
    columns.style.used_mut().reset_fragmentation.column_count = Some(4);
    assert!(!escaping_floats(&columns));
    let mut root = el("p", vec![floated()]);
    root.style.used_mut().reset_context.flow_root = Some(true);
    assert!(!escaping_floats(&el("div", vec![Node::Element(root)])));
    // The block itself floating always escapes.
    let mut float_root = columns;
    float_root.style.used_mut().reset_box_layout.float = Some(1);
    assert!(escaping_floats(&float_root));
}
