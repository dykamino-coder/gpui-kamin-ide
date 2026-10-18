//! Final selectedcontent reflects reset selection, including an empty listbox.

use super::tests::element;
use crate::{append, Handle, Node, NodeData, RcDom};
use markup5ever::interface::tree_builder::TreeSink;
use std::{cell::RefCell, rc::Rc};

fn text(value: &str) -> Handle {
    Node::new(NodeData::Text {
        contents: RefCell::new(value.into()),
    })
}

fn finish(select: Handle) -> RcDom {
    let dom = RcDom::default();
    append(&dom.document, select);
    dom.finish()
}

#[test]
fn selectedcontent_clones_last_selected_in_tree_order() {
    let target = element("selectedcontent", &[], vec![]);
    let select = element(
        "select",
        &[],
        vec![
            element("button", &[], vec![target.clone()]),
            element(
                "optgroup",
                &[],
                vec![element("option", &[("selected", "")], vec![text("A")])],
            ),
            element("option", &[("selected", "")], vec![text("B")]),
        ],
    );
    let _dom = finish(select);
    let children = target.children.borrow();
    let NodeData::Text { contents } = &children[0].data else {
        panic!("text expected")
    };
    assert_eq!(contents.borrow().as_ref(), "B");
}

#[test]
fn reset_without_selection_clears_previous_selectedcontent() {
    let stale = text("previous selection");
    let target = element("selectedcontent", &[], vec![stale.clone()]);
    let select = element(
        "select",
        &[("size", "2")],
        vec![
            element("button", &[], vec![target.clone()]),
            element("option", &[], vec![text("unselected")]),
        ],
    );
    let _dom = finish(select);
    assert!(target.children.borrow().is_empty());
    assert!(stale.parent().is_none());
}

#[test]
fn selectedcontent_target_is_first_in_tree_order() {
    let nested = element("selectedcontent", &[], vec![]);
    let direct = element("selectedcontent", &[], vec![]);
    let select = element(
        "select",
        &[],
        vec![element("button", &[], vec![nested.clone()]), direct],
    );
    assert!(Rc::ptr_eq(
        &select.get_a_selects_enabled_selectedcontent().unwrap(),
        &nested
    ));
}
