//! The kept box style of an atom: the same style while its inputs hold, a new
//! one when the viewport it read, its parent or its own style changes.

use super::{Stage, atom_style};
use crate::document::Document;
use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::BoxStyle;
use std::rc::Rc;

const PAGE: &str = "<!doctype html><style>.c{display:inline-block;width:8px;\
    background:#456}</style><p><b class=c></b><b class=c></b>\
    <i style='display:inline-block;width:10vw;background:#456'></i></p>";

fn all<'a>(nodes: &'a [Node], tag: &str, out: &mut Vec<&'a Element>) {
    for node in nodes {
        if let Node::Element(e) = node {
            if e.tag == tag {
                out.push(e);
            }
            all(&e.children, tag, out);
        }
    }
}

fn find<'a>(doc: &'a Document, tag: &str) -> Vec<&'a Element> {
    let mut out = Vec::new();
    all(&doc.nodes, tag, &mut out);
    out
}

fn opts(doc: &Document, viewport: (f32, f32)) -> RenderOpts {
    let opts = RenderOpts {
        text: gpui::TextStyle::default(),
        viewport,
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    opts.with_doc(doc.state.clone())
}

fn parent(doc: &Document) -> BoxStyle {
    // Materialized as a frame does: equal siblings share their computed style.
    let opts = opts(doc, (800., 600.));
    crate::style::cascade::materialize::materialize(
        &doc.nodes,
        &Rc::new(Default::default()),
        opts.cx.vcx(),
    );
    BoxStyle::shared(find(doc, "p")[0].style.computed())
}

/// The kept style equals a fresh computation at `opts`.
fn fresh(e: &Element, parent: &BoxStyle, opts: &RenderOpts, kept: &BoxStyle) {
    let mut fresh = box_style(parent, &e.style, opts.cx.vcx());
    fresh.resolve_viewport(opts.viewport, opts.cx.vcx());
    assert!(kept.same_box_values(&fresh));
}

#[test]
fn a_new_viewport_keeps_the_style_of_an_atom_that_does_not_read_it() {
    let doc = Document::builder(PAGE, "").build().unwrap();
    let parent = parent(&doc);
    let (wide, narrow) = (opts(&doc, (800., 600.)), opts(&doc, (400., 600.)));
    let b = find(&doc, "b")[0];
    let first = atom_style(b, &parent, &wide, Stage::Resolved);
    let again = atom_style(b, &parent, &narrow, Stage::Resolved);
    assert!(Rc::ptr_eq(&first, &again));
    fresh(b, &parent, &narrow, &again);
    let i = find(&doc, "i")[0];
    let first = atom_style(i, &parent, &wide, Stage::Resolved);
    let again = atom_style(i, &parent, &narrow, Stage::Resolved);
    assert!(!Rc::ptr_eq(&first, &again));
    fresh(i, &parent, &wide, &first);
    fresh(i, &parent, &narrow, &again);
}

#[test]
fn each_stage_and_each_atom_keeps_its_own_style() {
    let doc = Document::builder(PAGE, "").build().unwrap();
    let (parent, opts) = (parent(&doc), opts(&doc, (800., 600.)));
    let b = find(&doc, "b");
    let first = atom_style(b[0], &parent, &opts, Stage::Merged);
    let second = atom_style(b[1], &parent, &opts, Stage::Merged);
    // Equal siblings keep their own styles: the decorating box of each is itself.
    assert!(!Rc::ptr_eq(&first, &second));
    assert!(Rc::ptr_eq(
        &second,
        &atom_style(b[1], &parent, &opts, Stage::Merged)
    ));
    // Each stage keeps its own style.
    let resolved = atom_style(b[1], &parent, &opts, Stage::Resolved);
    assert!(!Rc::ptr_eq(&first, &resolved));
    assert!(Rc::ptr_eq(
        &resolved,
        &atom_style(b[1], &parent, &opts, Stage::Resolved)
    ));
}

#[test]
fn a_new_parent_or_own_style_computes_the_style_again() {
    let doc = Document::builder(PAGE, "").build().unwrap();
    let (parent, opts) = (parent(&doc), opts(&doc, (800., 600.)));
    let b = find(&doc, "b")[0];
    let first = atom_style(b, &parent, &opts, Stage::Resolved);
    let mut other = parent.clone();
    // A parent of another node: its absolute children resolve against another box.
    other.self_node = 77;
    let under_other = atom_style(b, &other, &opts, Stage::Resolved);
    assert!(!Rc::ptr_eq(&first, &under_other));
    fresh(b, &other, &opts, &under_other);
    let mut copy = b.clone();
    copy.style.used_mut().reset_box_layout.width = None;
    let own = atom_style(&copy, &parent, &opts, Stage::Resolved);
    assert!(!Rc::ptr_eq(&first, &own));
    fresh(&copy, &parent, &opts, &own);
}
