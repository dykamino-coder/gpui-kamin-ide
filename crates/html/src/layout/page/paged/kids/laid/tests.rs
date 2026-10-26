//! Независимые печатные группы: native dispatch и физические поля.

use super::*;
use crate::style::values::value::Len;

#[test]
fn empty_page_grid_uses_native_group_without_estimated_shape() {
    let tree = crate::dom::parse(
        "<div style='display:grid;grid-template-rows:20px 30px'></div>",
        "",
    );
    let nodes = [Node::Element(authored(&tree).clone())];
    let p = group(
        &nodes,
        &BoxStyle::default(),
        120.0,
        0.0,
        (false, false),
        tree.values(),
    )
    .unwrap();
    assert!(p.blocks);
    let Node::Element(e) = &p.nodes[0] else {
        panic!("grid")
    };
    let blocks = crate::layout::fragment::flow::blocks::BlockBox::collect(
        e,
        crate::layout::fragment::ShapeCx::COLUMNS,
        tree.values(),
    );
    assert!(!blocks.is_empty());
    assert!(blocks.grid.as_ref().unwrap().items.is_empty());
}

pub(super) fn authored(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("authored float")
}

#[test]
fn monolithic_page_float_keeps_its_measured_margin_box() {
    let tree = crate::dom::parse(
        "<div style='float:left;overflow:scroll;margin-top:20%;margin-bottom:10%;width:40px'>float</div>",
        "",
    );
    let nodes = [Node::Element(authored(&tree).clone())];
    let p = group(
        &nodes,
        &BoxStyle::default(),
        120.0,
        15.0,
        (false, false),
        tree.values(),
    )
    .expect("native monolithic float");
    assert!(!p.blocks);
    assert_eq!(p.margins, (0.0, 0.0, 0.0));
    let Node::Element(e) = &p.nodes[0] else {
        panic!("float")
    };
    assert_eq!(e.style.used().float, Some(-1));
    assert_eq!(e.style.used().margin.top, Some(Len::Px(24.0)));
    assert_eq!(e.style.used().margin.bottom, Some(Len::Px(12.0)));
}

#[test]
fn flowing_page_float_records_its_own_children_and_inline_continuation() {
    let tree = crate::dom::parse("<div style='float:left;width:20px'>flowing text</div>", "");
    let nodes = [Node::Element(authored(&tree).clone())];
    let p = group(
        &nodes,
        &BoxStyle::default(),
        120.0,
        0.0,
        (false, false),
        tree.values(),
    )
    .unwrap();
    assert!(p.blocks);
    assert_eq!(p.margins, (0.0, 0.0, 0.0));
    let Node::Element(e) = &p.nodes[0] else {
        panic!("float")
    };
    assert_eq!(e.style.used().float, Some(-1));
}

#[test]
fn empty_page_floats_use_their_layout_extent_even_when_the_pair_wraps() {
    let tree = crate::dom::parse(
        "<div style='width:100px;padding:5px'><div style='float:left;width:70px;height:40px'></div><div style='float:right;width:70px;height:60px'></div></div>",
        "",
    );
    let nodes = [Node::Element(authored(&tree).clone())];
    let p = group(
        &nodes,
        &BoxStyle::default(),
        120.0,
        0.0,
        (false, false),
        tree.values(),
    )
    .unwrap();
    assert!(p.blocks && p.float_extent);
    let Node::Element(e) = &p.nodes[0] else {
        panic!("float host")
    };
    assert_eq!(e.style.used().width, Some(Len::Px(100.0)));
}

#[test]
fn nonempty_float_extent_stops_at_an_inner_bfc() {
    for (context, expected) in [("", true), ("overflow:hidden", false)] {
        let tree = crate::dom::parse(
            &format!(
                "<div><div style='{context}'><div style='float:left;width:20px;height:300px'>text</div></div></div>"
            ),
            "",
        );
        let nodes = [Node::Element(authored(&tree).clone())];
        let p = group(
            &nodes,
            &BoxStyle::default(),
            120.,
            0.,
            (false, false),
            tree.values(),
        )
        .unwrap();
        assert_eq!(p.float_extent, expected);
    }
}
