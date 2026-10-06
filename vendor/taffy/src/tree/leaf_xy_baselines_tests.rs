//! Actual leaf callbacks export both physical axes without replacing content order.

use crate::style_helpers::*;
use crate::{Baselines, Contain, MeasureOutput, Size, Style, TaffyTree};

fn measured() -> MeasureOutput {
    MeasureOutput {
        size: Size { width: 40.0, height: 40.0 },
        baseline: Some(7.0),
        last_baseline: Some(17.0),
        baseline_x: Some(23.0),
        last_baseline_x: Some(11.0),
        baseline_x_from_right: false,
    }
}

fn style(contain: Contain) -> Style {
    let mut style = Style {
        size: Size { width: length(80.0), height: length(60.0) },
        contain,
        baseline_x_hint: Some((4.0, true)),
        ..Style::DEFAULT
    };
    style.padding.left = length(3.0);
    style.border.left = length(2.0);
    style.padding.top = length(4.0);
    style.border.top = length(1.0);
    style
}

fn layout(tree: &mut TaffyTree<()>, node: crate::NodeId) {
    tree.compute_layout_with_measure(node, Size::MAX_CONTENT, |input, _, _, style| {
        crate::compute_leaf_layout(input, style, |_, _| 0.0, |_, _| measured())
    }).unwrap();
}

#[test]
fn completed_leaf_preserves_both_axes_and_vertical_rl_order() {
    let mut tree = TaffyTree::new();
    tree.disable_rounding();
    let node = tree.new_leaf_with_context(style(Contain::NONE), ()).unwrap();
    layout(&mut tree, node);
    let output = tree.computed_layout_output(node).unwrap().unwrap();
    assert_eq!(output.baselines, Baselines { first: Some(12.0), last: Some(22.0) });
    assert_eq!(output.baselines_x, Baselines { first: Some(28.0), last: Some(16.0) });
    layout(&mut tree, node);
    assert_eq!(tree.computed_layout_output(node).unwrap(), Some(output));
    tree.mark_dirty(node).unwrap();
    assert_eq!(tree.computed_layout_output(node).unwrap(), None);
}

#[test]
fn containment_suppresses_both_axes_but_paint_keeps_real_baselines() {
    for contain in [Contain::LAYOUT, Contain::PAINT] {
        let mut tree = TaffyTree::new();
        let node = tree.new_leaf_with_context(style(contain), ()).unwrap();
        layout(&mut tree, node);
        let output = tree.computed_layout_output(node).unwrap().unwrap();
        if contain == Contain::LAYOUT {
            assert_eq!(output.baselines, Baselines::NONE);
            assert_eq!(output.baselines_x, Baselines::NONE);
        } else {
            assert_eq!(output.baselines, Baselines { first: Some(12.0), last: Some(22.0) });
            assert_eq!(output.baselines_x, Baselines { first: Some(28.0), last: Some(16.0) });
        }
    }
}

#[test]
fn legacy_size_measurement_has_no_content_baseline_on_either_axis() {
    let output: MeasureOutput = Size { width: 40.0, height: 30.0 }.into();
    assert_eq!(output.baseline, None);
    assert_eq!(output.last_baseline, None);
    assert_eq!(output.baseline_x, None);
    assert_eq!(output.last_baseline_x, None);
}

#[test]
fn right_origin_uses_final_content_width_instead_of_intrinsic_probe_width() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let mut style = style(Contain::NONE);
    style.padding.right = length(3.0);
    let node = tree.new_leaf_with_context(style, ()).unwrap();
    tree.compute_layout_with_measure(node, Size::MAX_CONTENT, |input, _, _, style| {
        crate::compute_leaf_layout(input, style, |_, _| 0.0, |_, _| MeasureOutput {
            baseline_x: Some(11.0),
            last_baseline_x: Some(23.0),
            baseline_x_from_right: true,
            ..measured()
        })
    }).unwrap();
    let output = tree.computed_layout_output(node).unwrap().unwrap();
    // Final width80, right padding3: first80-3-11 and last80-3-23.
    // Measured content width40 must not determine either coordinate.
    assert_eq!(output.baselines_x, Baselines { first: Some(66.0), last: Some(54.0) });
    assert_eq!(output.size.width, 80.0);
}
