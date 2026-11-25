//! The box executor (`repaint::rebox`): a batch whose damage rebuilds some
//! boxes gives the tree and the state of a fresh build of the new markup,
//! and builds only the subtree that contains what the change reaches; the
//! document is built only when the change reaches the document.

use super::super::Mutation;
use super::{state, tree};
use crate::document::Document;
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

/// The `n`-th element `local` of the source DOM, in tree order.
fn nth(node: &Handle, local: &str, n: &mut usize) -> Option<Handle> {
    if matches!(&node.data, NodeData::Element { name, .. } if &*name.local == local) {
        if *n == 0 {
            return Some(node.clone());
        }
        *n -= 1;
    }
    let children = node.children.borrow();
    children.iter().find_map(|c| nth(c, local, n))
}

fn set(element: &Handle, name: &str, value: Option<&str>) -> Mutation {
    Mutation::Attribute {
        element: element.clone(),
        name: html5ever::QualName::new(None, html5ever::ns!(), name.into()),
        value: value.map(Into::into),
    }
}

/// Apply the edits (element, its index among its kind, attribute, value)
/// to `old`: the revision equals a fresh build of `new`; `true` when the
/// document kept its values (no build of the document).
fn edit(old: &str, edits: &[(&str, usize, &str, Option<&str>)], new: &str) -> bool {
    let mut doc = Document::builder(old, "").build().unwrap();
    let values = doc.state.values.clone();
    let root = doc.source_dom();
    let records: Vec<Mutation> = edits
        .iter()
        .map(|&(local, n, name, value)| {
            let target = nth(&root, local, &mut { n }).expect("element");
            set(&target, name, value)
        })
        .collect();
    assert!(doc.apply(&records, "").unwrap().changed());
    let fresh = Document::builder(new, "").build().unwrap();
    assert_eq!(tree(&doc), tree(&fresh), "{old} -> {new}");
    assert_eq!(state(&doc), state(&fresh), "{old} -> {new}");
    Rc::ptr_eq(&values, &doc.state.values)
}

const STYLE: &str = "<!doctype html><html><head><style>body{margin:0}\
    p::before{content:attr(data-n) \". \"} .x{display:none} .b{display:block}\
    .t{display:table-cell} .w{width:50%}</style></head><body>";

fn page(body: &str) -> String {
    format!("{STYLE}{body}</body></html>")
}

#[test]
fn attr_in_generated_content_rebuilds_only_its_element() {
    let old = page("<div><p data-n=1>a</p><p data-n=2>b</p></div><p>c</p>");
    let new = old.replace("data-n=1", "data-n=7");
    assert!(edit(&old, &[("p", 0, "data-n", Some("7"))], &new));
}

#[test]
fn display_changes_rebuild_the_parent_of_the_box() {
    let old = page("<div><span class=x>a</span> b <i>c</i></div><p>d</p>");
    let new = old.replace(" class=x", "");
    assert!(edit(&old, &[("span", 0, "class", None)], &new));
    assert!(edit(&new, &[("span", 0, "class", Some("x"))], &old));
    let block = old.replace("class=x", "class=b");
    assert!(edit(&new, &[("span", 0, "class", Some("b"))], &block));
}

#[test]
fn list_items_renumber_from_their_list() {
    let old = page("<ul><li>a</li><li>b</li><li>c</li></ul><ol><li>d</li></ol>");
    let new = old.replace("<li>b", "<li class=b>b");
    assert!(edit(&old, &[("li", 1, "class", Some("b"))], &new));
    let hidden = old.replace("<li>b", "<li class=x>b");
    assert!(edit(&old, &[("li", 1, "class", Some("x"))], &hidden));
}

#[test]
fn nested_roots_take_the_outer_one() {
    let old = page("<div data-n=1><p data-n=2>a<span>s</span></p></div>");
    let new = old
        .replace("<div data-n=1>", "<div data-n=1 class=w>")
        .replace("data-n=2", "data-n=3");
    let edits = [
        ("div", 0, "class", Some("w")),
        ("p", 0, "data-n", Some("3")),
    ];
    assert!(edit(&old, &edits, &new));
}

#[test]
fn a_root_whose_parent_wraps_it_rebuilds_the_parent() {
    let old = page("<div><span>a</span><span>b</span></div><p>c</p>");
    let new = old.replace("<span>a", "<span class=t>a");
    assert!(edit(&old, &[("span", 0, "class", Some("t"))], &new));
    assert!(edit(&new, &[("span", 0, "class", None)], &old));
}

#[test]
fn a_layout_change_rebuilds_its_element() {
    let old = page("<div><p data-n=1>a</p></div><section><p>b</p></section>");
    let new = old.replace("<section>", "<section class=w>");
    assert!(edit(&old, &[("section", 0, "class", Some("w"))], &new));
}
