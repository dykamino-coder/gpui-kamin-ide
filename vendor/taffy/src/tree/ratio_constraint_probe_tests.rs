//! Preferred aspect ratios cannot override explicit height or maximum height.
use crate::prelude::*;

fn layout(height: Dimension, maximum: crate::LengthPercentageAuto) -> Size<f32> {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let node = tree
        .new_leaf(Style {
            display: Display::Block,
            size: Size {
                width: length(200.0),
                height,
            },
            max_size: Size {
                width: auto(),
                height: maximum,
            },
            aspect_ratio: Some(1.0),
            ..Style::DEFAULT
        })
        .unwrap();
    tree.compute_layout(node, Size::MAX_CONTENT).unwrap();
    tree.layout(node).unwrap().size
}

#[test]
fn explicit_height_takes_priority_over_the_preferred_ratio() {
    assert_eq!(
        layout(length(100.0), auto()),
        Size {
            width: 200.0,
            height: 100.0
        }
    );
}

#[test]
fn maximum_height_limits_ratio_derived_automatic_height() {
    assert_eq!(
        layout(auto(), length(100.0)),
        Size {
            width: 200.0,
            height: 100.0
        }
    );
}

#[test]
fn unconstrained_automatic_height_still_uses_the_preferred_ratio() {
    assert_eq!(
        layout(auto(), auto()),
        Size {
            width: 200.0,
            height: 200.0
        }
    );
}
