//! A float lifted out of an undecorated inline: its element, the values the
//! inline hands down and the containing-block facts of the block.

use super::*;
use crate::style::box_input::BoxOrigin;
use crate::style::cascade::materialize::materialize;
use crate::style::computed::Computed;
use std::rc::Rc;

#[test]
fn lifted_float_keeps_its_element_inline_values_and_block_facts() {
    let parsed = crate::dom::parse(
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
    // The block stays the containing block of the percentage height.
    let containing = box_style(&block_box, &original.style, vcx);
    assert_eq!(
        lifted.style.used().cb_height_def_hint,
        containing.cb_height_def
    );
    assert_eq!(
        lifted.style.used().quirk_pct_base_hint,
        containing.quirk_pct_base
    );
    // Re-entering under the block keeps the inline's inherited color.
    let Node::Element(span) = &nodes[0] else {
        panic!("leading inline fragment")
    };
    let span_box = box_style(&block_box, &span.style, vcx);
    let lifted_box = box_style(&block_box, &lifted.style, vcx);
    assert_eq!(lifted_box.color, span_box.color);
    assert_ne!(lifted_box.color, block_box.color);
}
