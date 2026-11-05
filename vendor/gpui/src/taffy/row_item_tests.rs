//! Baseline of a row's single item: bottom margin edge without a content baseline, nothing for
//! a non-participating or out-of-flow item.
use super::*;
use taffy::style_helpers::TaffyMaxContent;

fn sized(height: f32, top: f32, bottom: f32, align_self: Option<taffy::AlignSelf>) -> taffy::Style {
    taffy::Style {
        size: taffy::Size {
            width: taffy::Dimension::length(8.),
            height: taffy::Dimension::length(height),
        },
        margin: taffy::Rect {
            left: taffy::LengthPercentageAuto::length(0.),
            right: taffy::LengthPercentageAuto::length(0.),
            top: taffy::LengthPercentageAuto::length(top),
            bottom: taffy::LengthPercentageAuto::length(bottom),
        },
        align_self,
        ..Default::default()
    }
}

fn laid_row(item: taffy::Style) -> (TaffyTree<NodeContext>, NodeId) {
    let mut tree = TaffyTree::new();
    tree.disable_rounding();
    let item = tree.new_leaf(item).unwrap();
    let row = tree
        .new_with_children(
            taffy::Style {
                display: taffy::Display::Flex,
                align_items: taffy::AlignItems::BASELINE,
                ..Default::default()
            },
            &[item],
        )
        .unwrap();
    tree.compute_layout(row, taffy::Size::MAX_CONTENT).unwrap();
    (tree, row)
}

#[test]
fn item_without_content_baseline_aligns_by_its_bottom_margin_edge() {
    let (tree, row) = laid_row(sized(20., 3., 5., None));
    let got = row_item(&tree, row, 2.).unwrap();
    // Device pixels halve at scale 2: border box 20 plus both margins.
    assert_eq!(got.outer_height, 14.);
    assert_eq!(got.baseline, Some(14.));
}

#[test]
fn item_aligned_otherwise_has_no_baseline_but_keeps_its_height() {
    let (tree, row) = laid_row(sized(20., 0., 0., Some(taffy::AlignSelf::FLEX_START)));
    let got = row_item(&tree, row, 1.).unwrap();
    assert_eq!(got.outer_height, 20.);
    assert_eq!(got.baseline, None);
}

#[test]
fn row_without_in_flow_item_has_none() {
    let (tree, row) = laid_row(taffy::Style {
        position: taffy::Position::Absolute,
        ..sized(20., 0., 0., None)
    });
    assert_eq!(row_item(&tree, row, 1.), None);
}
