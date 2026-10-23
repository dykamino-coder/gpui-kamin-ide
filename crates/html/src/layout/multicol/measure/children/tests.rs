//! Отступы меры копии и атомарный отказ до регистрации ребёнка.

use super::*;
use crate::Document;

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
    let doc = Document::builder("<!doctype html><div>x</div>", "").build();
    let mut c = element(doc.nodes()).clone();
    let kids = RefCell::new(Vec::new());
    let whole = RefCell::new(Vec::new());
    for (top, bottom, expected) in [
        (None, Some(Len::Auto), Some((0.0, 0.0))),
        (Some(Len::Px(-3.0)), Some(Len::Px(7.0)), Some((-3.0, 7.0))),
        (Some(Len::Pct(10.0)), None, None),
        (None, Some(Len::Pct(10.0)), None),
    ] {
        c.style.margin.top = top;
        c.style.margin.bottom = bottom;
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
    .build();
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
        &Computed::default(),
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
