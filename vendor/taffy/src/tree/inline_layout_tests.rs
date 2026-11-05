//! Контракт атомарной коробки: реальные поля, ограничения и baseline без flex-родителя.

use super::*;
use crate::{geometry::Point, style_helpers::*, Baselines, MeasureOutput, Rect};

fn measured(inputs: LayoutInput, style: &Style) -> LayoutOutput {
    compute_leaf_layout(inputs, style, calc_value, |known, _| MeasureOutput {
        size: known.unwrap_or(Size { width: 60.0, height: 20.0 }),
        baseline: Some(12.0),
        last_baseline: Some(16.0),
        ..Size::ZERO.into()
    })
}

#[test]
fn atomic_inline_uses_its_own_node_and_border_box_coordinates() {
    let mut tree = TaffyTree::<()>::new();
    tree.disable_rounding();
    let node = tree.new_leaf(Style {
        margin: Rect { left: length(3.0), right: length(7.0), top: length(5.0), bottom: length(9.0) },
        inset: Rect { left: length(2.0), top: length(-4.0), ..auto() },
        position: crate::Position::Relative,
        ..Default::default()
    }).unwrap();
    tree.compute_inline_with_measure(node, Size::MAX_CONTENT, |inputs, _, _, style| measured(inputs, style)).unwrap();
    assert_eq!(tree.total_node_count(), 1);
    assert_eq!(tree.parent(node), None);
    let layout = tree.layout(node).unwrap();
    assert_eq!(layout.size, Size { width: 60.0, height: 20.0 });
    assert_eq!(layout.location, Point { x: 5.0, y: 1.0 });
    assert_eq!(layout.relative_offset, Point { x: 2.0, y: -4.0 });
    let output = tree.computed_layout_output(node).unwrap().unwrap();
    assert_eq!(output.baselines, Baselines { first: Some(12.0), last: Some(16.0) });
}

#[test]
fn available_space_excludes_margins_but_percentage_basis_does_not() {
    let mut tree = TaffyTree::<()>::new();
    let node = tree.new_leaf(Style {
        margin: Rect { left: percent(0.1), right: percent(0.2), ..zero() },
        ..Default::default()
    }).unwrap();
    let mut called = false;
    tree.compute_inline_with_measure(node, Size { width: AvailableSpace::Definite(100.0), height: AvailableSpace::MaxContent },
        |inputs, _, _, style| {
            called = true;
            assert_eq!(inputs.parent_size.width, Some(100.0));
            assert_eq!(inputs.available_space.width, AvailableSpace::Definite(70.0));
            assert_eq!(inputs.known_dimensions, Size::NONE);
            assert_eq!(inputs.vertical_margins_are_collapsible, crate::geometry::Line::FALSE);
            measured(inputs, style)
        }).unwrap();
    assert!(called);
    assert_eq!(tree.layout(node).unwrap().margin.left, 10.0);
    assert_eq!(tree.layout(node).unwrap().margin.right, 20.0);
}

#[test]
fn shrink_to_fit_relayout_keeps_box_sizing_and_baselines() {
    let mut tree = TaffyTree::<()>::new();
    tree.disable_rounding();
    let node = tree.new_leaf(Style {
        box_sizing: crate::BoxSizing::ContentBox,
        padding: length(2.0),
        border: length(1.0),
        ..Default::default()
    }).unwrap();
    tree.compute_inline_with_measure(node, Size::MAX_CONTENT, |inputs, _, _, style| measured(inputs, style)).unwrap();
    assert_eq!(tree.layout(node).unwrap().size.width, 66.0);
    let mut style = tree.style(node).unwrap().clone();
    style.max_size.width = length(30.0);
    tree.set_style(node, style).unwrap();
    let available = Size { width: AvailableSpace::Definite(36.0), height: AvailableSpace::MaxContent };
    tree.compute_inline_with_measure(node, available, |inputs, _, _, style| measured(inputs, style)).unwrap();
    assert_eq!(tree.layout(node).unwrap().size.width, 36.0);
    let output = tree.computed_layout_output(node).unwrap().unwrap();
    assert_eq!(output.baselines.first, Some(15.0));
    assert_eq!(output.baselines.last, Some(19.0));
    // A repeated performed pass must retain descendant geometry and output.
    tree.compute_inline_with_measure(node, available, |_, _, _, _| panic!("performed layout should be cached")).unwrap();
    assert_eq!(tree.layout(node).unwrap().size.width, 36.0);
}

#[cfg(feature = "block_layout")]
#[test]
fn orthogonal_atomic_edges_keep_the_containing_blocks_percentage_basis() {
    let mut tree = TaffyTree::<()>::new();
    tree.disable_rounding();
    let node = tree.new_leaf(Style {
        block_flow: Some(crate::BlockFlow { vertical: true, ..Default::default() }),
        margin: percent(0.1),
        padding: percent(0.02),
        ..Default::default()
    }).unwrap();
    let available = Size { width: AvailableSpace::Definite(100.0), height: AvailableSpace::MaxContent };
    tree.compute_inline_with_measure(node, available, |inputs, _, _, style| measured(inputs, style)).unwrap();
    let layout = tree.layout(node).unwrap();
    assert_eq!(layout.margin, Rect { left: 10.0, right: 10.0, top: 10.0, bottom: 10.0 });
    assert_eq!(layout.padding, Rect { left: 2.0, right: 2.0, top: 2.0, bottom: 2.0 });
    assert_eq!(layout.location, Point { x: 10.0, y: 10.0 });
    assert_eq!(layout.size, Size { width: 64.0, height: 24.0 });
}
