//! A hoisted absolutely positioned box keeps its element and the values its
//! CSS parent hands down; only its box parent changes.

use super::*;
use crate::style::box_input::BoxOrigin;
use crate::style::cascade::materialize::{box_style, materialize};
use crate::style::computed::Computed;
use std::rc::Rc;

#[test]
fn hoisted_absolute_box_keeps_its_element_and_css_parent_values() {
    let parsed = crate::dom::parse(
        r#"<div id="block" style="color: red; font-size: 10px"><span style="color: green; font-size: 20px">a<b id="abs" style="position: absolute; left: 0; top: 0">x</b>c</span></div>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let block =
        crate::dom::test_support::find(&parsed.nodes, &|e| e.attr("id") == Some("block")).unwrap();
    let original =
        crate::dom::test_support::find(&parsed.nodes, &|e| e.attr("id") == Some("abs")).unwrap();
    let block_box = box_style(&BoxStyle::default(), &block.style, vcx);
    let nodes = hoist_inset_abs(&block.children, &block_box, vcx);
    let [Node::Element(span), Node::Element(moved)] = nodes.as_slice() else {
        panic!("the absolute box follows its inline: {nodes:?}");
    };
    assert_eq!(span.tag, "span");
    assert!(span.children.iter().all(|n| !matches!(n, Node::Element(_))));
    // Identity: the same element, children and marks, now a moved box.
    assert_eq!(moved.node_id, original.node_id);
    assert_eq!(moved.attr("id"), Some("abs"));
    assert_eq!(
        format!("{:?}", moved.children),
        format!("{:?}", original.children)
    );
    assert_eq!(moved.marks.box_origin, Some(BoxOrigin::Moved));
    assert_eq!(original.marks.box_origin, None);
    // Re-entering under the block keeps the inline's inherited values.
    let span_box = box_style(&block_box, &span.style, vcx);
    let moved_box = box_style(&block_box, &moved.style, vcx);
    assert_eq!(moved_box.color, span_box.color);
    assert_ne!(moved_box.color, block_box.color);
    assert_eq!(moved_box.font_size, span_box.font_size);
    // The element's own style is left as it was.
    assert!(original.style.published().is_some());
}
