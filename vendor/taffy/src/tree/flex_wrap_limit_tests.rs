//! A fixed maximum still constrains wrapping after an intrinsic parent probe.
use crate::geometry::Line;
use crate::tree::{LayoutInput, LayoutPartialTree, RequestedAxis, RunMode, SizingMode};
use crate::{
    AvailableSpace, Dimension, FlexDirection, FlexWrap, LengthPercentageAuto, Size, Style,
    TaffyTree,
};

fn wrapping(definite: bool, maximum: Option<f32>) -> (f32, f32, f32) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let first = tree
        .new_leaf(Style {
            size: Size::from_lengths(50.0, 50.0),
            flex_grow: 1.0,
            ..Style::DEFAULT
        })
        .unwrap();
    let second = tree
        .new_leaf(Style {
            size: Size::from_lengths(50.0, 100.0),
            ..Style::DEFAULT
        })
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                flex_direction: FlexDirection::Column,
                flex_wrap: FlexWrap::Wrap,
                size: Size {
                    width: Dimension::length(100.0),
                    height: Dimension::auto(),
                },
                max_size: Size {
                    width: LengthPercentageAuto::auto(),
                    height: maximum
                        .map(LengthPercentageAuto::length)
                        .unwrap_or(LengthPercentageAuto::auto()),
                },
                ..Style::DEFAULT
            },
            &[first, second],
        )
        .unwrap();
    tree.as_layout_tree().compute_child_layout(
        root,
        LayoutInput {
            known_dimensions: Size {
                width: Some(100.0),
                height: Some(100.0),
            },
            known_dimensions_are_definite: Size {
                width: true,
                height: definite,
            },
            parent_size: Size::NONE,
            available_space: Size {
                width: AvailableSpace::Definite(100.0),
                height: AvailableSpace::Definite(100.0),
            },
            sizing_mode: SizingMode::InherentSize,
            axis: RequestedAxis::Both,
            run_mode: RunMode::PerformLayout,
            vertical_margins_are_collapsible: Line::FALSE,
        },
    );
    (
        tree.layout(first).unwrap().size.height,
        tree.layout(second).unwrap().location.x,
        tree.layout(second).unwrap().location.y,
    )
}

#[test]
fn fixed_maximum_wraps_despite_content_derived_known_height() {
    assert_eq!(wrapping(false, Some(100.0)), (100.0, 50.0, 0.0));
}

#[test]
fn definite_known_height_still_wraps_at_the_same_maximum() {
    assert_eq!(wrapping(true, Some(100.0)), (100.0, 50.0, 0.0));
}

#[test]
fn intrinsic_known_height_without_fixed_maximum_remains_one_line() {
    let (_, x, _) = wrapping(false, None);
    assert_eq!(x, 0.0);
}
