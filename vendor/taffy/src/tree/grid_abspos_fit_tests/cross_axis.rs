//! An intrinsic grid contribution must not minimize an indefinite perpendicular axis.
use crate::prelude::*;
use crate::{compute_leaf_layout, AlignItems};

fn contribution(vertical: bool, cross: Option<f32>) -> f32 {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let text = tree.new_leaf_with_context(Style::DEFAULT, ()).unwrap();
    let cross_size = cross.map(length).unwrap_or(auto());
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: if vertical {
                    Size {
                        width: auto(),
                        height: cross_size,
                    }
                } else {
                    Size {
                        width: cross_size,
                        height: auto(),
                    }
                },
                justify_items: Some(AlignItems::START),
                align_items: Some(AlignItems::START),
                ..Style::DEFAULT
            },
            &[text],
        )
        .unwrap();
    let available = if vertical {
        Size {
            width: AvailableSpace::MinContent,
            height: AvailableSpace::MaxContent,
        }
    } else {
        Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MinContent,
        }
    };
    tree.compute_layout_with_measure(root, available, |input, _, _, style| {
        compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |known, available| {
                let (known_along, available_along) = if vertical {
                    (known.height, available.height)
                } else {
                    (known.width, available.width)
                };
                let along = known_along.unwrap_or(match available_along {
                    AvailableSpace::MinContent => 40.0,
                    AvailableSpace::Definite(value) => value.min(75.0),
                    AvailableSpace::MaxContent => 75.0,
                });
                let across = if along < 75.0 { 40.0 } else { 20.0 };
                if vertical {
                    Size {
                        width: known.width.unwrap_or(across),
                        height: along,
                    }
                } else {
                    Size {
                        width: along,
                        height: known.height.unwrap_or(across),
                    }
                }
            },
        )
    })
    .unwrap();
    let size = tree.layout(root).unwrap().size;
    if vertical {
        size.width
    } else {
        size.height
    }
}

#[test]
fn indefinite_perpendicular_axis_does_not_force_wrapping() {
    assert_eq!(contribution(true, None), 20.0);
    assert_eq!(contribution(false, None), 20.0);
}

#[test]
fn definite_perpendicular_axis_still_constrains_wrapping() {
    for vertical in [true, false] {
        assert_eq!(contribution(vertical, Some(100.0)), 20.0);
        assert_eq!(contribution(vertical, Some(60.0)), 40.0);
    }
}
