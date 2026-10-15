//! Regression coverage for containing-block coordinates and parallel extents.

use super::*;
use crate::dom::{Element, Node};
use crate::layout::fragment::fragment_size::shape_full;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

fn element(style: Computed, children: Vec<Node>) -> Element {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    Element {
        list_item: None,
        node_id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        anim: None,
        tag: "div".into(),
        style,
        hover: None,
        first_letter: None,
        first_line: None,
        children,
        attrs: Vec::new(),
        inline: false,
    }
}

#[test]
fn positioned_reach_crosses_static_wrappers_without_moving_siblings() {
    use crate::style::computed::Position;
    let abs = element(
        Computed {
            height: Some(Len::Px(200.0)),
            position: Some(Position::Absolute),
            ..Default::default()
        },
        vec![],
    );
    let wrapper = element(Computed::default(), vec![Node::Element(abs)]);
    let sibling = element(
        Computed {
            height: Some(Len::Px(20.0)),
            ..Default::default()
        },
        vec![],
    );
    let parent = element(
        Computed {
            position: Some(Position::Relative),
            ..Default::default()
        },
        vec![Node::Element(wrapper), Node::Element(sibling)],
    );
    let shape = shape_full(&parent, 4, ShapeCx::COLUMNS).unwrap();
    assert_eq!(shape.0, 200.0);
    assert_eq!(
        OOF_OWN.with(|m| m.borrow().get(&parent.node_id).copied()),
        Some((20.0, 200.0))
    );
}

#[test]
fn explicit_top_uses_continuous_containing_block_origin_in_columns() {
    use crate::style::computed::Position;
    let first = element(
        Computed {
            height: Some(Len::Px(100.0)),
            ..Default::default()
        },
        vec![],
    );
    let mut abs = element(
        Computed {
            height: Some(Len::Px(200.0)),
            position: Some(Position::Absolute),
            ..Default::default()
        },
        vec![],
    );
    abs.style.inset.top = Some(Len::Px(0.0));
    let parent = element(
        Computed {
            position: Some(Position::Relative),
            ..Default::default()
        },
        vec![Node::Element(first), Node::Element(abs)],
    );
    assert_eq!(shape_full(&parent, 4, ShapeCx::COLUMNS).unwrap().0, 200.0);
}

#[test]
fn contained_fixed_reach_passes_relative_boxes_to_its_own_cb() {
    use crate::style::computed::Position;
    let mut fixed = element(
        Computed {
            height: Some(Len::Px(200.0)),
            position: Some(Position::Fixed),
            ..Default::default()
        },
        vec![],
    );
    fixed.style.inset.top = Some(Len::Px(0.0));
    let relative = element(
        Computed {
            position: Some(Position::Relative),
            ..Default::default()
        },
        vec![Node::Element(fixed)],
    );
    let parent = element(
        Computed {
            contain_layout: Some(true),
            ..Default::default()
        },
        vec![Node::Element(relative)],
    );
    assert_eq!(shape_full(&parent, 4, ShapeCx::COLUMNS).unwrap().0, 200.0);
}
