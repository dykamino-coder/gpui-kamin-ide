//! Отступы меры копии и атомарный отказ до регистрации ребёнка.

use super::*;
use crate::Document;
use crate::style::computed::{BoxStyle, Computed};

fn element(nodes: &[Node]) -> &Element {
    nodes
        .iter()
        .find_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
        .expect("document has an element")
}

#[test]
fn measured_margins_keep_auto_zero_and_reject_unresolved_lengths() {
    let doc = Document::builder("<!doctype html><div>x</div>", "")
        .build()
        .unwrap();
    let mut c = element(doc.nodes()).clone();
    let kids = RefCell::new(Vec::new());
    let whole = RefCell::new(Vec::new());
    for (top, bottom, expected) in [
        (None, Some(Len::Auto), Some((0.0, 0.0))),
        (Some(Len::Px(-3.0)), Some(Len::Px(7.0)), Some((-3.0, 7.0))),
        (Some(Len::Pct(10.0)), None, None),
        (None, Some(Len::Pct(10.0)), None),
    ] {
        c.style.used_mut().margin.top = top;
        c.style.used_mut().margin.bottom = bottom;
        kids.borrow_mut().clear();
        whole.borrow_mut().clear();
        assert_eq!(measured_margins(&c), expected);
        let result = measured_nested(&c, 123.0, &kids, &whole);
        if let Some((mt, mb)) = expected {
            let (_, shape) = result.expect("resolved margins");
            assert_eq!((shape.margin_top, shape.margin_bottom), (mt, mb));
            assert_eq!(*kids.borrow(), vec![(c.node_id, 123.0)]);
            assert_eq!(*whole.borrow(), vec![c.node_id]);
        } else {
            assert!(result.is_none());
            assert!(kids.borrow().is_empty());
            assert!(whole.borrow().is_empty());
        }
    }
}

#[test]
fn flex_avoid_boundaries_keep_solver_line_membership() {
    let doc = Document::builder(
        "<div><div style='display:flex;flex-flow:column wrap;height:200px'>\
         <div style='height:80px;break-inside:avoid'></div>\
         <div style='height:150px;break-before:avoid'></div></div></div>",
        "",
    )
    .build()
    .unwrap();
    let parent = element(doc.nodes());
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let measured = RefCell::new(Vec::new());
    let result = measure_children(
        parent,
        &BoxStyle::new(Computed::default()),
        &opts,
        None,
        None,
        None,
        (Some(50.0), Some(50.0)),
        &RefCell::new(Vec::new()),
        false,
        &RefCell::new(Vec::new()),
        &measured,
        false,
        false,
        true,
        true,
    )
    .expect("measured flex subtree");
    assert_eq!(result.len(), 1);
    assert!(!result[0].0.marks.paginate);
    assert_eq!(*measured.borrow(), vec![(result[0].0.node_id, 50.0)]);
}

#[test]
fn wrapped_multicol_uses_its_laid_height_instead_of_summing_inner_columns() {
    let doc = Document::builder(
        "<div><div><div style='columns:2;column-height:20px;column-wrap:wrap;row-gap:10px'>\
         <div style='height:20px'></div><div style='height:20px'></div>\
         <div style='height:20px'></div><div style='height:20px'></div>\
         </div></div></div>",
        "",
    )
    .build()
    .unwrap();
    let parent = element(doc.nodes());
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let measured = RefCell::default();
    let kids = measure_children(
        parent,
        &BoxStyle::default(),
        &opts,
        None,
        None,
        None,
        (Some(100.0), Some(100.0)),
        &RefCell::default(),
        true,
        &RefCell::default(),
        &measured,
        false,
        false,
        false,
        true,
    )
    .expect("independently laid-out multicol descendant");
    assert_eq!(kids.len(), 1);
    assert!(!kids[0].0.marks.paginate);
    assert_eq!(
        kids[0].1.height, 0.0,
        "height is deferred to the copy layout"
    );
    assert_eq!(*measured.borrow(), vec![(kids[0].0.node_id, 100.0)]);
}

#[test]
fn direct_multicol_keeps_its_outer_fragmentation_dispatch() {
    fn authored(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|node| match node {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => authored(&e.children),
            _ => None,
        })
    }
    for style in [
        "columns:2;height:200px",
        "columns:2;column-fill:auto",
        "columns:2;column-height:20px;column-wrap:wrap",
    ] {
        let doc = Document::builder(
            &format!("<div style='{style}'><div style='height:300px'></div></div>"),
            "",
        )
        .build()
        .unwrap();
        let c = authored(doc.nodes()).expect("authored multicol");
        assert!(crate::layout::multicol::spanner::multicol_container(
            &c.style.used()
        ));
        assert!(!super::independent_kid::gate(c, true, Some(100.0)));
        assert!(!super::independent_kid::own_layout_gate(
            c,
            true,
            Some(100.0)
        ));
    }
}
