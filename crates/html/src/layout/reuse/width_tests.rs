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
    assert_eq!(independent(&fixed("div", 300.0, vec![]), vcx), Width::Fixed);
    assert_eq!(
        independent(&fixed("table", 300.0, vec![]), vcx),
        Width::Fixed
    );
    assert_eq!(independent(&el("div", vec![]), vcx), Width::Floor);
    let text = Node::Text(crate::dom::Text::from_source("row".into(), None));
    assert_eq!(independent(&el("div", vec![text]), vcx), Width::Stretch);
    let inner = Node::Element(el("span", vec![]));
    assert_eq!(independent(&el("div", vec![inner]), vcx), Width::Floor);
    let mut centered = el("div", vec![]);
    centered.style.used_mut().inh.text_flow.text_align =
        Some(crate::style::computed::TextAlign::Center);
    assert_eq!(independent(&centered, vcx), Width::Floor);
    assert_eq!(independent(&el("table", vec![]), vcx), Width::Floor);
    let mut pct = el("div", vec![]);
    pct.style.used_mut().reset_box_layout.width = Some(Len::Pct(50.0));
    assert_eq!(independent(&pct, vcx), Width::Dependent);
}

#[test]
fn an_auto_width_block_is_reusable_only_while_its_floor_holds_it_wider_than_the_flow() {
    // Filled flow: the width is the flow's, any other flow changes it.
    assert_eq!(reusable_range(Width::Floor, 800.0, Some(800.0), None), None);
    // An overflowing table at its min-content: the same up to that width.
    let floor = Some((0.0, 1018.0));
    assert_eq!(
        reusable_range(Width::Floor, 1018.0, Some(800.0), None),
        floor
    );
    assert_eq!(reusable_range(Width::Floor, 1018.0, None, None), None);
    let any = Some((0.0, f32::INFINITY));
    assert_eq!(reusable_range(Width::Fixed, 300.0, None, None), any);
    assert_eq!(
        reusable_range(Width::Dependent, 1018.0, Some(800.0), None),
        None
    );
}

#[test]
fn a_one_line_text_block_is_reusable_while_its_line_fits() {
    // The line leaves 200 of 800: it stays one line down to a 601 flow.
    let range = reusable_range(Width::Stretch, 800.0, Some(800.0), Some(200.0));
    assert_eq!(range, Some((601.0, f32::INFINITY)));
    assert_eq!(
        reusable_range(Width::Stretch, 800.0, Some(800.0), None),
        None,
        "wrapped"
    );
    assert_eq!(
        reusable_range(Width::Stretch, 800.0, Some(800.0), Some(0.5)),
        None,
        "flush"
    );
}

#[test]
fn percentage_margins_or_viewport_lengths_inside_make_a_block_width_dependent() {
    let doc = DocumentValues::detached(Default::default());
    let vcx = doc.values();
    let mut margin = fixed("div", 300.0, vec![]);
    margin.style.used_mut().reset_box_layout.margin.left = Some(Len::Pct(10.0));
    assert_eq!(independent(&margin, vcx), Width::Dependent);
    let mut padding = fixed("div", 300.0, vec![]);
    padding.style.used_mut().reset_box_layout.padding.top = Some(Len::Vh(1.0));
    assert_eq!(independent(&padding, vcx), Width::Dependent);
    let mut inner = el("div", vec![]);
    inner.style.used_mut().reset_box_layout.width = Some(Len::Vw(10.0));
    let outer = fixed("div", 300.0, vec![Node::Element(inner)]);
    assert_eq!(
        independent(&outer, vcx),
        Width::Dependent,
        "a descendant sized by the viewport"
    );
}
