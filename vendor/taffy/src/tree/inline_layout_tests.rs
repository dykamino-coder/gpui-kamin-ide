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
    let node = tree
        .new_leaf(Style {
            margin: Rect { left: length(3.0), right: length(7.0), top: length(5.0), bottom: length(9.0) },
            inset: Rect { left: length(2.0), top: length(-4.0), ..auto() },
            position: crate::Position::Relative,
            ..Default::default()
        })
        .unwrap();
    tree.compute_inline_with_measure(node, Size::MAX_CONTENT, None, |inputs, _, _, style| measured(inputs, style))
        .unwrap();
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
fn relative_insets_use_the_hosts_projection_instead_of_the_atoms_direction() {
    for (direction, left, expected) in [
        // LTR containing block: its left inset wins even for an RTL atom.
        (crate::Direction::Rtl, length(2.0), 2.0),
        // RTL containing block: the host has removed the losing left inset.
        (crate::Direction::Ltr, auto(), -7.0),
    ] {
        let mut tree = TaffyTree::<()>::new();
        let node = tree
            .new_leaf(Style {
                direction,
                inset: Rect { left, right: length(7.0), ..auto() },
                position: crate::Position::Relative,
                ..Default::default()
            })
            .unwrap();
        tree.compute_inline_with_measure(node, Size::MAX_CONTENT, None, |inputs, _, _, style| measured(inputs, style))
            .unwrap();
        assert_eq!(tree.layout(node).unwrap().relative_offset.x, expected);
        assert_eq!(tree.layout(node).unwrap().location.x, expected);
    }
}

#[test]
fn available_space_excludes_margins_but_percentage_basis_does_not() {
    let mut tree = TaffyTree::<()>::new();
    let node = tree
        .new_leaf(Style { margin: Rect { left: percent(0.1), right: percent(0.2), ..zero() }, ..Default::default() })
        .unwrap();
    let mut called = false;
    tree.compute_inline_with_measure(
        node,
        Size { width: AvailableSpace::Definite(100.0), height: AvailableSpace::MaxContent },
        None,
        |inputs, _, _, style| {
            called = true;
            assert_eq!(inputs.parent_size.width, Some(100.0));
            assert_eq!(inputs.available_space.width, AvailableSpace::Definite(70.0));
            assert_eq!(inputs.known_dimensions, Size::NONE);
            assert_eq!(inputs.vertical_margins_are_collapsible, crate::geometry::Line::FALSE);
            measured(inputs, style)
        },
    )
    .unwrap();
    assert!(called);
    assert_eq!(tree.layout(node).unwrap().margin.left, 10.0);
    assert_eq!(tree.layout(node).unwrap().margin.right, 20.0);
}

#[test]
fn shrink_to_fit_relayout_keeps_box_sizing_and_baselines() {
    let mut tree = TaffyTree::<()>::new();
    tree.disable_rounding();
    let node = tree
        .new_leaf(Style {
            box_sizing: crate::BoxSizing::ContentBox,
            padding: length(2.0),
            border: length(1.0),
            ..Default::default()
        })
        .unwrap();
    tree.compute_inline_with_measure(node, Size::MAX_CONTENT, None, |inputs, _, _, style| measured(inputs, style))
        .unwrap();
    assert_eq!(tree.layout(node).unwrap().size.width, 66.0);
    let mut style = tree.style(node).unwrap().clone();
    style.max_size.width = length(30.0);
    tree.set_style(node, style).unwrap();
    let available = Size { width: AvailableSpace::Definite(36.0), height: AvailableSpace::MaxContent };
    tree.compute_inline_with_measure(node, available, None, |inputs, _, _, style| measured(inputs, style)).unwrap();
    assert_eq!(tree.layout(node).unwrap().size.width, 36.0);
    let output = tree.computed_layout_output(node).unwrap().unwrap();
    assert_eq!(output.baselines.first, Some(15.0));
    assert_eq!(output.baselines.last, Some(19.0));
    // A repeated performed pass must retain descendant geometry and output.
    tree.compute_inline_with_measure(node, available, None, |_, _, _, _| panic!("performed layout should be cached"))
        .unwrap();
    assert_eq!(tree.layout(node).unwrap().size.width, 36.0);
}

#[cfg(feature = "block_layout")]
#[test]
fn orthogonal_atomic_edges_keep_the_containing_blocks_percentage_basis() {
    let mut tree = TaffyTree::<()>::new();
    tree.disable_rounding();
    let node = tree
        .new_leaf(Style {
            block_flow: Some(crate::BlockFlow { vertical: true, ..Default::default() }),
            margin: percent(0.1),
            padding: percent(0.02),
            ..Default::default()
        })
        .unwrap();
    let available = Size { width: AvailableSpace::Definite(100.0), height: AvailableSpace::MaxContent };
    tree.compute_inline_with_measure(node, available, None, |inputs, _, _, style| measured(inputs, style)).unwrap();
    let layout = tree.layout(node).unwrap();
    assert_eq!(layout.margin, Rect { left: 10.0, right: 10.0, top: 10.0, bottom: 10.0 });
    assert_eq!(layout.padding, Rect { left: 2.0, right: 2.0, top: 2.0, bottom: 2.0 });
    assert_eq!(layout.location, Point { x: 10.0, y: 10.0 });
    assert_eq!(layout.size, Size { width: 64.0, height: 24.0 });
}

#[test]
fn selected_shrink_to_fit_width_does_not_shrink_again_to_the_longest_line() {
    let mut tree = TaffyTree::<()>::new();
    tree.disable_rounding();
    let node = tree
        .new_leaf(Style {
            box_sizing: crate::BoxSizing::ContentBox,
            margin: Rect { left: length(3.0), right: length(7.0), ..zero() },
            padding: length(2.0),
            border: length(1.0),
            ..Default::default()
        })
        .unwrap();
    let available = Size { width: AvailableSpace::Definite(320.0), height: AvailableSpace::MaxContent };
    let measure = |inputs: LayoutInput, _, _: Option<&mut ()>, style: &Style| {
        assert_eq!(inputs.parent_size.width, Some(320.0));
        compute_leaf_layout(inputs, style, calc_value, |_, _| MeasureOutput {
            // Wrapped text's longest rendered line need not fill the selected width.
            size: Size { width: 137.0, height: 40.0 },
            baseline: Some(32.0),
            ..Size::ZERO.into()
        })
    };
    tree.compute_inline_with_measure(node, available, None, measure).unwrap();
    assert_eq!(tree.layout(node).unwrap().size.width, 143.0);
    tree.compute_inline_with_measure(node, available, Some(160.0), measure).unwrap();
    let layout = tree.layout(node).unwrap();
    assert_eq!(layout.size.width, 150.0);
    assert_eq!(layout.size.width + layout.margin.horizontal_axis_sum(), 160.0);
    assert_eq!(tree.computed_layout_output(node).unwrap().unwrap().baselines.first, Some(35.0));
    assert!(tree.style(node).unwrap().size.width.is_auto());
    assert!(tree.style(node).unwrap().max_size.width.is_auto());
    assert_eq!(tree.total_node_count(), 1);
    // Selecting another width must be part of the layout cache key.
    tree.compute_inline_with_measure(node, available, Some(140.0), measure).unwrap();
    assert_eq!(tree.layout(node).unwrap().size.width, 130.0);
}
