//! An inherited paint value of an ancestor (`html.alt body{color}`) reaches the
//! whole page: the revision repaints in place, also through boxes with hover
//! layers, and equals a fresh build of the same markup.

use super::repaint_background::computed;
use super::{state, tree};
use crate::document::Document;
use crate::dom::Node;
use std::rc::Rc;

const PAGE: &str = "<!doctype html><html@><head><style>\
    html,body{margin:0;color:#000;background:#fff}html.alt body{color:#111}a:hover{color:#00f}td{padding:1px}</style></head>\
    <body><p>a <a href=x>b</a></p><div><span>c</span><a href=y>d</a></div>\
    @tail</body></html>";

fn page(alt: bool, tail: &str) -> String {
    PAGE.replace(
        "<html@>",
        if alt {
            "<html class=\"alt\">"
        } else {
            "<html>"
        },
    )
    .replace("@tail", tail)
}

/// The cascaded hover layers of the boxes, in tree order.
fn hovers(doc: &Document) -> String {
    fn walk(nodes: &[Node], out: &mut String) {
        for node in nodes {
            if let Node::Element(e) = node {
                if let Some(h) = &e.hover {
                    out.push_str(&format!("{} {:?}\n", e.tag, h.cascaded()));
                }
                walk(&e.children, out);
            }
        }
    }
    let mut out = String::new();
    walk(doc.nodes(), &mut out);
    out
}

/// Toggle the root class three times; revisions taken in place.
fn run(tail: &str) -> (usize, Vec<String>) {
    let before = crate::paint_path::read();
    let page = |alt| page(alt, tail);
    let mut doc = Document::builder(&page(false), "").build().unwrap();
    computed(&doc);
    assert!(!hovers(&doc).is_empty(), "the links have hover layers");
    let mut in_place = 0;
    for alt in [true, false, true] {
        let values = doc.state.values.clone();
        assert!(doc.update(&page(alt), "").unwrap().changed());
        in_place += usize::from(Rc::ptr_eq(&values, &doc.state.values));
        let fresh = Document::builder(&page(alt), "").build().unwrap();
        assert_eq!(computed(&doc), computed(&fresh), "alt {alt}");
        assert_eq!(hovers(&doc), hovers(&fresh), "alt {alt}");
        assert_eq!(tree(&doc), tree(&fresh), "alt {alt}");
        assert_eq!(state(&doc), state(&fresh), "alt {alt}");
    }
    let after = crate::paint_path::read();
    let names = crate::paint_path::NAMES
        .iter()
        .zip(before.iter().zip(after));
    let moved = names.filter(|(_, (b, a))| **b != *a);
    (
        in_place,
        moved.map(|(n, (b, a))| format!("{n}+{}", a - b)).collect(),
    )
}

#[test]
fn an_inherited_colour_repaints_in_place_through_hover_layers_and_a_table() {
    let (n, why) = run("<table><tr><td>1</td><td><a href=z>2</a></td></tr></table>");
    assert_eq!(n, 3, "{why:?}");
}
