//! Column x-baseline groups affect intrinsic size and honor auto-margin exclusions.
use crate::prelude::*;
use crate::{AlignItems, Baselines};

fn layout(spec: &[(f32, f32, f32, u8, AlignItems, bool)], width: Dimension) -> (f32, Vec<f32>) {
    let mut tree: TaffyTree<(f32, f32, f32)> = TaffyTree::new();
    tree.disable_rounding();
    let mut items = Vec::new();
    for &(width, first, last, flags, alignment, auto_margin) in spec {
        items.push(
            tree.new_leaf_with_context(
                Style {
                    baseline_x_flags: flags,
                    align_self: Some(alignment),
                    margin: Rect {
                        left: if auto_margin { auto() } else { zero() },
                        ..Rect::zero()
                    },
                    ..Style::DEFAULT
                },
                (width, first, last),
            )
            .unwrap(),
        );
    }
    let root = tree
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                size: Size {
                    width,
                    height: auto(),
                },
                ..Style::DEFAULT
            },
            &items,
        )
        .unwrap();
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |inputs, _, context, style| {
        let (width, first, last) = *context.unwrap();
        let mut output = crate::compute_leaf_layout(
            inputs,
            style,
            |_, _| 0.0,
            |known, _| Size {
                width: known.width.unwrap_or(width),
                height: known.height.unwrap_or(20.0),
            },
        );
        output.baselines_x = Baselines {
            first: Some(first),
            last: Some(last),
        };
        output
    })
    .unwrap();
    (
        tree.layout(root).unwrap().size.width,
        items
            .iter()
            .map(|&item| tree.layout(item).unwrap().location.x)
            .collect(),
    )
}

#[test]
fn intrinsic_column_width_includes_baseline_overhang() {
    assert_eq!(
        layout(
            &[
                (20.0, 18.0, 18.0, 4, AlignItems::BASELINE, false),
                (30.0, 4.0, 4.0, 4, AlignItems::BASELINE, false),
            ],
            auto()
        ),
        (44.0, vec![0.0, 14.0])
    );
}

#[test]
fn opposite_flow_and_opposite_preference_share_one_physical_group() {
    assert_eq!(
        layout(
            &[
                (20.0, 6.0, 14.0, 5, AlignItems::BASELINE, false),
                (30.0, 9.0, 24.0, 4, AlignItems::LAST_BASELINE, false),
            ],
            length(80.0)
        ),
        (80.0, vec![60.0, 42.0])
    );
}

#[test]
fn auto_cross_margin_does_not_create_a_baseline_sharing_group() {
    assert_eq!(
        layout(
            &[
                (20.0, 6.0, 14.0, 4, AlignItems::BASELINE, false),
                (30.0, 9.0, 24.0, 4, AlignItems::BASELINE, true),
            ],
            length(80.0)
        ),
        (80.0, vec![0.0, 50.0])
    );
}

#[test]
fn orthogonal_items_share_synthesized_central_baselines() {
    for (flags, alignment, positions) in [
        (2, AlignItems::BASELINE, vec![5.0, 0.0]),
        (3, AlignItems::BASELINE, vec![55.0, 50.0]),
        (2, AlignItems::LAST_BASELINE, vec![55.0, 50.0]),
        (3, AlignItems::LAST_BASELINE, vec![5.0, 0.0]),
    ] {
        assert_eq!(
            layout(
                &[
                    (20.0, 3.0, 17.0, flags, alignment, false),
                    (30.0, 4.0, 26.0, flags, alignment, false),
                ],
                length(80.0)
            ),
            (80.0, positions)
        );
    }
}
