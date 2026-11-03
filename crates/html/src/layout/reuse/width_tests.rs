use super::*;
use crate::dom::Marks;
use crate::style::computed::Computed;
use crate::values::DocumentValues;

fn el(tag: &str, children: Vec<Node>) -> Element {
    Element::synthetic(tag, Computed::default(), Marks::default(), children)
}

fn fixed(tag: &str, px: f32, children: Vec<Node>) -> Element {
    let mut e = el(tag, children);
    e.style.used_mut().reset_box_layout.width = Some(Len::Px(px));
    e
}

#[test]
fn only_a_fixed_width_block_is_independent_of_the_flow_width() {
    let doc = DocumentValues::detached(Default::default());
    let vcx = doc.values();
    assert!(independent(&fixed("div", 300.0, vec![]), vcx));
    assert!(independent(&fixed("table", 300.0, vec![]), vcx));
    assert!(
        !independent(&el("div", vec![]), vcx),
        "auto width fills the flow"
    );
    assert!(
        !independent(&el("table", vec![]), vcx),
        "our auto table box follows the flow"
    );
    let mut pct = el("div", vec![]);
    pct.style.used_mut().reset_box_layout.width = Some(Len::Pct(50.0));
    assert!(!independent(&pct, vcx));
}

#[test]
fn percentage_margins_or_viewport_lengths_inside_make_a_block_width_dependent() {
    let doc = DocumentValues::detached(Default::default());
    let vcx = doc.values();
    let mut margin = fixed("div", 300.0, vec![]);
    margin.style.used_mut().reset_box_layout.margin.left = Some(Len::Pct(10.0));
    assert!(!independent(&margin, vcx));
    let mut padding = fixed("div", 300.0, vec![]);
    padding.style.used_mut().reset_box_layout.padding.top = Some(Len::Vh(1.0));
    assert!(!independent(&padding, vcx));
    let mut inner = el("div", vec![]);
    inner.style.used_mut().reset_box_layout.width = Some(Len::Vw(10.0));
    let outer = fixed("div", 300.0, vec![Node::Element(inner)]);
    assert!(
        !independent(&outer, vcx),
        "a descendant sized by the viewport"
    );
}
