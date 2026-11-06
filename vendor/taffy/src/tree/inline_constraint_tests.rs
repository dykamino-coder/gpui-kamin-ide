//! Used atomic sizes constrain descendants before their independent layout.
use super::*;
use crate::{style_helpers::*, Display, MeasureOutput, Position, Rect};

fn empty(inputs: LayoutInput, _: NodeId, _: Option<&mut ()>, style: &Style) -> LayoutOutput {
    compute_leaf_layout(inputs, style, calc_value, |known, _| MeasureOutput {
        size: known.unwrap_or(Size::ZERO),
        ..Size::ZERO.into()
    })
}

#[test]
fn definite_atomic_size_performs_one_layout_without_intrinsic_probes() {
    let mut tree = TaffyTree::new();
    let node = tree.new_leaf(Style {
        size: Size { width: length(80.0), height: length(20.0) },
        ..Style::DEFAULT
    }).unwrap();
    let mut calls = 0;
    tree.compute_inline_with_measure(node, Size::MAX_CONTENT, None, |inputs, id, context, style| {
        calls += 1;
        assert_eq!(inputs.run_mode, crate::RunMode::PerformLayout);
        assert_eq!(inputs.known_dimensions, Size { width: Some(80.0), height: Some(20.0) });
        empty(inputs, id, context, style)
    }).unwrap();
    assert_eq!(calls, 1);
    assert_eq!(tree.total_node_count(), 1);
}

#[cfg(feature = "block_layout")]
#[test]
fn clamped_atomic_height_is_the_percentage_containing_block() {
    let mut tree = TaffyTree::new();
    let child = tree.new_leaf(Style {
        size: Size { width: length(100.0), height: percent(1.0) },
        ..Style::DEFAULT
    }).unwrap();
    let node = tree.new_with_children(Style {
        display: Display::Block,
        size: Size { width: length(100.0), height: length(200.0) },
        max_size: Size { width: auto(), height: length(100.0) },
        ..Style::DEFAULT
    }, &[child]).unwrap();
    tree.compute_inline_with_measure(node, Size::MAX_CONTENT, None, empty).unwrap();
    assert_eq!(tree.layout(node).unwrap().size.height, 100.0);
    assert_eq!(tree.layout(child).unwrap().size.height, 100.0);
}

#[test]
fn orthogonal_text_measure_receives_its_definite_inline_extent() {
    let mut tree = TaffyTree::<()>::new();
    let node = tree.new_leaf(Style {
        size: Size { width: auto(), height: length(120.0) },
        ..Style::DEFAULT
    }).unwrap();
    tree.compute_inline_with_measure(node, Size::MAX_CONTENT, None, |inputs, _, _, style| {
        // A vertical text fitter needs the used inline extent before measuring
        // its content-dependent physical width.
        assert_eq!(inputs.known_dimensions.height, Some(120.0));
        compute_leaf_layout(inputs, style, calc_value, |_, _| MeasureOutput::from(Size { width: 40.0, height: 120.0 }))
    }).unwrap();
    assert_eq!(tree.layout(node).unwrap().size, Size { width: 40.0, height: 120.0 });
}

#[cfg(feature = "grid")]
#[test]
fn atomic_grid_auto_repeat_uses_intrinsic_width_before_ratio_height() {
    let mut tree = TaffyTree::new();
    let node = tree.new_with_children(Style {
        display: Display::Grid,
        aspect_ratio: Some(1.0),
        min_size: Size { width: auto(), height: length(60.0) },
        grid_template_columns: vec![repeat(crate::RepetitionCount::AutoFill, vec![length(50.0)])],
        ..Style::DEFAULT
    }, &[]).unwrap();
    tree.compute_inline_with_measure(node, Size::MAX_CONTENT, None, empty).unwrap();
    assert_eq!(tree.layout(node).unwrap().size, Size { width: 100.0, height: 100.0 });
}

#[cfg(feature = "grid")]
#[test]
fn fractional_atomic_grid_keeps_content_sized_block_axis_indefinite() {
    let mut tree = TaffyTree::new();
    let item = tree.new_leaf(Style { size: length(200.0), ..Style::DEFAULT }).unwrap();
    let abs = tree.new_leaf(Style {
        position: Position::Absolute,
        size: percent(2.0),
        grid_column: crate::Line { start: line(1), end: line(2) },
        grid_row: crate::Line { start: line(1), end: line(2) },
        inset: Rect { left: length(0.0), top: length(0.0), ..auto() },
        ..Style::DEFAULT
    }).unwrap();
    let node = tree.new_with_children(Style {
        display: Display::Grid,
        position: Position::Relative,
        grid_template_columns: vec![minmax(length(0.0), fr(0.5))],
        grid_template_rows: vec![minmax(length(0.0), fr(0.5))],
        ..Style::DEFAULT
    }, &[item, abs]).unwrap();
    tree.compute_inline_with_measure(node, Size::MAX_CONTENT, None, empty).unwrap();
    assert_eq!(tree.layout(node).unwrap().size, Size { width: 100.0, height: 100.0 });
    assert_eq!(tree.layout(abs).unwrap().size, Size { width: 100.0, height: 100.0 });
}

#[cfg(feature = "flexbox")]
#[test]
fn atomic_table_preserves_its_own_minimum_width_algorithm() {
    let mut tree = TaffyTree::new();
    let child = tree.new_leaf(Style { size: length(100.0), ..Style::DEFAULT }).unwrap();
    let node = tree.new_with_children(Style {
        display: Display::Flex,
        item_is_table: true,
        size: Size { width: length(20.0), height: auto() },
        ..Style::DEFAULT
    }, &[child]).unwrap();
    tree.compute_inline_with_measure(node, Size::MAX_CONTENT, None, empty).unwrap();
    assert_eq!(tree.layout(node).unwrap().size.width, 100.0);
}
