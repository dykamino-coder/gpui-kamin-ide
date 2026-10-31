//! A float lifted out of an undecorated inline: its element, the values the
//! inline hands down and the containing-block facts of the block.

use super::*;
use crate::style::box_input::BoxOrigin;
use crate::style::cascade::materialize::materialize;
use crate::style::computed::Computed;
use std::rc::Rc;

#[test]
fn lifted_float_keeps_its_element_inline_values_and_block_facts() {
    let parsed = crate::document::parsed::parse(
        r#"<div id="block" style="color: red; height: 50px"><span style="color: green">a<b id="float" style="float: left; height: 50%">x</b>c</span></div>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let block =
        crate::dom::test_support::find(&parsed.nodes, &|e| e.attr("id") == Some("block")).unwrap();
    let original =
        crate::dom::test_support::find(&parsed.nodes, &|e| e.attr("id") == Some("float")).unwrap();
    let block_box = box_style(&BoxStyle::default(), &block.style, vcx);
    let nodes = lift(block.children.to_vec(), &block_box, vcx);
    let lifted: Vec<&Element> = nodes
        .iter()
        .filter_map(|n| match n {
            Node::Element(e) if e.attr("id") == Some("float") => Some(e),
            _ => None,
        })
        .collect();
    let [lifted] = lifted.as_slice() else {
        panic!("one lifted float: {nodes:?}");
    };
    assert_eq!(nodes.len(), 3, "inline fragments around the float");
    assert_eq!(lifted.node_id, original.node_id);
    assert_eq!(lifted.marks.box_origin, Some(BoxOrigin::Moved));
    assert_eq!(original.marks.box_origin, None);
    // The element keeps its own cascaded and computed style.
    assert!(std::ptr::eq(
        lifted.style.cascaded(),
        original.style.cascaded()
    ));
    assert!(lifted.style.published().is_some());
    // The block stays the containing block of the percentage height.
    let containing = box_style(&block_box, &original.style, vcx);
    let moved = box_style(&block_box, &lifted.style, vcx);
    assert_eq!(moved.cb_height_def, containing.cb_height_def);
    assert_eq!(moved.quirk_pct_base, containing.quirk_pct_base);
    // Re-entering under the block keeps the inline's inherited color.
    let Node::Element(span) = &nodes[0] else {
        panic!("leading inline fragment")
    };
    let span_box = box_style(&block_box, &span.style, vcx);
    let lifted_box = box_style(&block_box, &lifted.style, vcx);
    assert_eq!(lifted_box.color, span_box.color);
    assert_ne!(lifted_box.color, block_box.color);
    // Its cascade holds only its own declarations: the inline's colour is
    // inherited, not declared on the float (CSS Cascade 5 §4.2).
    assert!(lifted.style.cascaded().color.is_none());
}

#[test]
fn lifted_float_takes_tree_values_from_the_inline_and_box_facts_from_the_block() {
    let parsed = crate::document::parsed::parse(
        r#"<div id="block" style="overflow: hidden; height: 40px"><span id="inline" style="user-select: none">a<b id="float" style="float: left">x</b>c</span></div>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let find = |id| {
        crate::dom::test_support::find(&parsed.nodes, &|e: &Element| e.attr("id") == Some(id))
            .unwrap()
    };
    let block_box = box_style(&BoxStyle::default(), &find("block").style, vcx);
    let span_box = box_style(&block_box, &find("inline").style, vcx);
    let in_place = box_style(&span_box, &find("float").style, vcx);
    let nodes = lift(find("block").children.to_vec(), &block_box, vcx);
    let lifted = nodes
        .iter()
        .find_map(|n| match n {
            Node::Element(e) if e.attr("id") == Some("float") => Some(e),
            _ => None,
        })
        .unwrap();
    let lifted_box = box_style(&block_box, &lifted.style, vcx);
    // css-ui-4 §6.1: the used `user-select` follows the parent element.
    assert_eq!(in_place.no_select, Some(true));
    assert_eq!(lifted_box.no_select, in_place.no_select);
    // CSS 2 §9.5, §10.1: the block is the box parent; its clip encloses the float.
    assert!(lifted_box.fragment_clip_ancestor);
}

#[test]
fn lifted_float_colors_pass_the_subtree_filter_once() {
    let parsed = crate::document::parsed::parse(
        r#"<div id="block" style="filter: invert(1); color: rgb(0, 0, 255)"><span>a<b id="float" style="float: left; background: rgb(0, 128, 0); border: 1px solid rgb(255, 0, 0)">x</b>c</span></div>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let block =
        crate::dom::test_support::find(&parsed.nodes, &|e| e.attr("id") == Some("block")).unwrap();
    let original =
        crate::dom::test_support::find(&parsed.nodes, &|e| e.attr("id") == Some("float")).unwrap();
    let block_box = box_style(&BoxStyle::default(), &block.style, vcx);
    let Node::Element(span) = &block.children[0] else {
        panic!("inline")
    };
    // Filter Effects 1 §5: the filter paints the element and its subtree
    // once; in place, the float's colours pass through it once.
    let in_place = box_style(
        &box_style(&block_box, &span.style, vcx),
        &original.style,
        vcx,
    );
    let nodes = lift(block.children.to_vec(), &block_box, vcx);
    let lifted = nodes
        .iter()
        .find_map(|n| match n {
            Node::Element(e) if e.attr("id") == Some("float") => Some(e),
            _ => None,
        })
        .unwrap();
    let lifted_box = box_style(&block_box, &lifted.style, vcx);
    let (lifted, in_place) = (lifted_box.paint.unwrap(), in_place.paint.unwrap());
    assert_eq!(lifted.background, in_place.background);
    assert_eq!(lifted.foreground, in_place.foreground);
    assert_eq!(lifted.border_colors, in_place.border_colors);
}
