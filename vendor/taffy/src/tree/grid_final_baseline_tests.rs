//! KaminIDE patch: табличная baseline берётся из итоговой раскладки без повторного margin.
use crate::prelude::*;
use crate::{AvailableSpace, MeasureOutput};

#[test]
fn table_cell_margin_is_added_only_at_placement() {
    let mut tree: TaffyTree<f32> = TaffyTree::new();
    tree.disable_rounding();
    let first = tree
        .new_leaf_with_context(
            Style {
                size: Size::from_lengths(40.0, 40.0),
                table_cell_baseline: true,
                align_self: Some(AlignItems::BASELINE),
                margin: Rect {
                    top: length(5.0),
                    ..Rect::zero()
                },
                ..Style::DEFAULT
            },
            10.0,
        )
        .unwrap();
    let second = tree
        .new_leaf_with_context(
            Style {
                size: Size::from_lengths(40.0, 40.0),
                table_cell_baseline: true,
                align_self: Some(AlignItems::BASELINE),
                ..Style::DEFAULT
            },
            20.0,
        )
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                grid_template_columns: vec![length(40.0), length(40.0)],
                grid_template_rows: vec![length(60.0)],
                ..Style::DEFAULT
            },
            &[first, second],
        )
        .unwrap();
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, context, style| {
        crate::compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |_, _| MeasureOutput {
                size: Size {
                    width: 40.0,
                    height: 40.0,
                },
                baseline: context.as_deref().copied(),
                last_baseline: context.as_deref().copied(),
                ..Size::ZERO.into()
            },
        )
    })
    .unwrap();
    let result = tree.computed_layout_output(root).unwrap().unwrap();
    assert_eq!(result.baselines.first, Some(20.0));
}

#[test]
fn grid_exports_final_child_metrics_instead_of_track_probe() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let cell = tree
        .new_leaf_with_context(
            Style {
                table_cell_baseline: true,
                align_self: Some(AlignItems::BASELINE),
                ..Style::DEFAULT
            },
            (),
        )
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                grid_template_columns: vec![length(80.0)],
                grid_template_rows: vec![length(60.0)],
                ..Style::DEFAULT
            },
            &[cell],
        )
        .unwrap();
    let mut probes = 0;
    let mut finals = 0;
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, _, style| {
        let probe = input.available_space.height == AvailableSpace::MinContent;
        if probe {
            probes += 1;
        } else {
            finals += 1;
        }
        crate::compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |_, _| MeasureOutput {
                size: Size {
                    width: 40.0,
                    height: 40.0,
                },
                baseline: Some(if probe { 3.0 } else { 19.0 }),
                last_baseline: Some(if probe { 5.0 } else { 29.0 }),
                ..Size::ZERO.into()
            },
        )
    })
    .unwrap();
    assert!(probes > 0 && finals > 0);
    let result = tree.computed_layout_output(root).unwrap().unwrap();
    assert_eq!(result.baselines.first, Some(19.0));
    assert_eq!(result.baselines.last, Some(19.0));
}
