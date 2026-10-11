//! Real nested grids must export the final child's available baseline set.
use crate::geometry::{Line, Rect, Size};
use crate::style_helpers::{TaffyAuto, TaffyMaxContent};
use crate::tree::{Baselines, LayoutPartialTreeExt, SizingMode};
use crate::{AlignItems, Dimension, Display, LengthPercentageAuto, TaffyTree};
type Style = crate::Style;

fn nested(align: AlignItems, root_height: Option<f32>, top_margin: f32) -> Baselines {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let leaf = tree
        .new_leaf(Style {
            size: Size {
                width: Dimension::length(10.0),
                height: Dimension::length(20.0),
            },
            ..Style::DEFAULT
        })
        .unwrap();
    let child = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size {
                    width: Dimension::length(100.0),
                    height: Dimension::length(100.0),
                },
                margin: Rect {
                    top: LengthPercentageAuto::length(top_margin),
                    right: LengthPercentageAuto::length(0.0),
                    bottom: LengthPercentageAuto::length(0.0),
                    left: LengthPercentageAuto::length(0.0),
                },
                ..Style::DEFAULT
            },
            &[leaf],
        )
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                align_items: align,
                size: Size {
                    width: Dimension::length(100.0),
                    height: root_height
                        .map(Dimension::length)
                        .unwrap_or(Dimension::AUTO),
                },
                ..Style::DEFAULT
            },
            &[child],
        )
        .unwrap();
    let result = tree
        .as_layout_tree()
        .perform_child_layout(
            root,
            Size::NONE,
            Size::NONE,
            Size::MAX_CONTENT,
            SizingMode::InherentSize,
            Line::FALSE,
        )
        .baselines;
    result
}

#[test]
fn nested_grid_exports_available_final_child_baselines() {
    assert_eq!(
        nested(AlignItems::START, None, 0.0),
        Baselines {
            first: Some(20.0),
            last: Some(20.0)
        }
    );
}
#[test]
fn nested_last_baseline_sharing_supplies_the_first_container_set() {
    assert_eq!(
        nested(AlignItems::LAST_BASELINE, Some(150.0), 0.0),
        Baselines {
            first: Some(70.0),
            last: Some(70.0)
        }
    );
}
#[test]
fn nested_grid_margin_translates_the_set_once() {
    assert_eq!(
        nested(AlignItems::START, None, 10.0),
        Baselines {
            first: Some(30.0),
            last: Some(30.0)
        }
    );
}

#[test]
#[cfg(feature = "flexbox")]
fn empty_table_wrapper_does_not_export_its_empty_grid_shim() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let shim = tree
        .new_leaf(Style {
            display: Display::Grid,
            ..Style::DEFAULT
        })
        .unwrap();
    let table = tree
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: crate::FlexDirection::Column,
                size: Size::from_lengths(60.0, 60.0),
                item_is_table: true,
                baseline_unavailable: true,
                ..Style::DEFAULT
            },
            &[shim],
        )
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                ..Style::DEFAULT
            },
            &[table],
        )
        .unwrap();
    let result = tree.as_layout_tree().perform_child_layout(
        root,
        Size::NONE,
        Size::NONE,
        Size::MAX_CONTENT,
        SizingMode::InherentSize,
        Line::FALSE,
    );
    assert_eq!(
        result.baselines,
        Baselines {
            first: Some(60.0),
            last: Some(60.0)
        }
    );
}
