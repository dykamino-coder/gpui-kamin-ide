//! Raw subtree text borrows the retained IFC buffer of a flat text flow.

use crate::document::Document;
use crate::dom::{Element, Node};
use crate::layout::cx::{BuildDoc, LayoutCx, ValueContext};
use crate::style::computed::BoxStyle;
use crate::text::content::{Content, raw};
use std::rc::Rc;

fn paragraph(nodes: &[Node]) -> &Element {
    let mut pending = vec![nodes.iter()];
    while let Some(siblings) = pending.last_mut() {
        match siblings.next() {
            Some(Node::Element(e)) if e.tag == "p" => return e,
            Some(Node::Element(e)) => pending.push(e.children.iter()),
            Some(Node::Text(_)) => {}
            None => {
                pending.pop();
            }
        }
    }
    panic!("paragraph");
}

/// The layout context of `doc` after its box tree is finished.
fn finished(doc: &Document) -> LayoutCx {
    doc.state
        .boxes
        .borrow_mut()
        .finish(&doc.nodes, &BoxStyle::default(), false);
    LayoutCx {
        values: ValueContext {
            doc: BuildDoc::of(doc.state.clone()),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn raw_buffer_is_shared_before_normalization_and_survives_text_commit() {
    let html = "<p style='text-transform:uppercase'>  a  b </p>";
    let mut doc = Document::builder(html, "").build().unwrap();
    let cx = finished(&doc);
    let p = paragraph(&doc.nodes);
    let first = raw(&p.children, &cx);
    let second = raw(&p.children, &cx);
    let (Content::Retained(a), Content::Retained(b)) = (&first, &second) else {
        panic!("a flat flow must use the retained owner");
    };
    assert!(Rc::ptr_eq(a, b));
    assert_eq!(&*first, "  a  b ");
    doc.update("<p style='text-transform:uppercase'>changed</p>", "")
        .unwrap();
    assert_eq!(&*first, "  a  b ");
    let cx = finished(&doc);
    assert_eq!(&*raw(&paragraph(&doc.nodes).children, &cx), "changed");
}

#[test]
fn opaque_descendants_and_synthetic_slices_keep_full_subtree_text() {
    let html = "<p>before <span style='display:inline-block'>inside</span> after</p>";
    let doc = Document::builder(html, "").build().unwrap();
    let cx = finished(&doc);
    let mixed = raw(&paragraph(&doc.nodes).children, &cx);
    assert!(matches!(mixed, Content::Gathered(_)));
    assert_eq!(&*mixed, "before inside after");
    let synthetic = vec![Node::Text("synthetic".into())];
    assert_eq!(&*raw(&synthetic, &cx), "synthetic");
}
