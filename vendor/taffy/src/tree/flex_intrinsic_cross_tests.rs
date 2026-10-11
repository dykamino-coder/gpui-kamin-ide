//! Intrinsic cross keywords constrain main-basis probes before ratio transfer.
use crate::style_helpers::TaffyMaxContent;
use crate::{AvailableSpace, Dimension, Display, FlexDirection, Size, Style, TaffyTree};

pub(super) fn column(width: Dimension) -> Style {
    Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        size: Size {
            width,
            height: Dimension::auto(),
        },
        ..Style::DEFAULT
    }
}

fn nested_ratio(outer_width: f32, green_width: Dimension) -> (Size<f32>, Size<f32>) {
    nested_ratio_case(
        Dimension::length(outer_width),
        green_width,
        100.0,
        AvailableSpace::MaxContent,
    )
}

fn nested_ratio_case(
    outer_width: Dimension,
    green_width: Dimension,
    leaf_height: f32,
    available_width: AvailableSpace,
) -> (Size<f32>, Size<f32>) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let leaf = tree
        .new_leaf(Style {
            size: Size::from_lengths(100.0, leaf_height),
            ..column(Dimension::length(100.0))
        })
        .unwrap();
    let ratio = tree
        .new_with_children(
            Style {
                aspect_ratio: Some(1.0),
                ..column(Dimension::auto())
            },
            &[leaf],
        )
        .unwrap();
    let green = tree
        .new_with_children(column(green_width), &[ratio])
        .unwrap();
    let outer = tree
        .new_with_children(column(outer_width), &[green])
        .unwrap();
    tree.compute_layout(
        outer,
        Size {
            width: available_width,
            height: AvailableSpace::MaxContent,
        },
    )
    .unwrap();
    (
        tree.layout(green).unwrap().size,
        tree.layout(ratio).unwrap().size,
    )
}

#[test]
fn intrinsic_cross_keyword_preserves_nested_ratio_main_contribution() {
    for outer in [300.0, 784.0, 900.0] {
        for keyword in [Dimension::min_content(), Dimension::max_content()] {
            let (green, ratio) = nested_ratio(outer, keyword);
            assert_eq!(
                green,
                Size {
                    width: 100.0,
                    height: 100.0
                },
                "outer={outer}, keyword={keyword:?}"
            );
            assert_eq!(
                ratio,
                Size {
                    width: 100.0,
                    height: 100.0
                }
            );
        }
    }
}

#[test]
fn automatic_cross_size_keeps_real_stretch_ratio_transfer() {
    for outer in [300.0, 784.0, 900.0] {
        let (green, ratio) = nested_ratio(outer, Dimension::auto());
        assert_eq!(
            green,
            Size {
                width: outer,
                height: outer
            }
        );
        assert_eq!(ratio, green);
    }
}

#[test]
fn fit_percentage_does_not_make_a_false_known_cross_ratio_basis() {
    let (green, ratio) = nested_ratio_case(
        Dimension::length(200.0),
        Dimension::fit_content_percent(0.1),
        0.0,
        AvailableSpace::MaxContent,
    );
    assert_eq!(
        green,
        Size {
            width: 100.0,
            height: 100.0
        }
    );
    assert_eq!(ratio, green);
}

#[test]
fn explicit_stretch_and_indefinite_percentage_keep_automatic_cross_behavior() {
    for width in [300.0, 784.0, 900.0] {
        let (green, ratio) = nested_ratio(width, Dimension::stretch());
        assert_eq!(
            green,
            Size {
                width,
                height: width
            }
        );
        assert_eq!(ratio, green);
        let (green, ratio) = nested_ratio_case(
            Dimension::auto(),
            Dimension::percent(1.0),
            100.0,
            AvailableSpace::Definite(width),
        );
        assert_eq!(
            green,
            Size {
                width,
                height: width
            }
        );
        assert_eq!(ratio, green);
    }
}

#[test]
fn intrinsic_cross_probe_preserves_a_definite_main_percentage_basis() {
    for keyword in [Dimension::min_content(), Dimension::max_content()] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let leaf = tree
            .new_leaf(Style {
                size: Size::from_lengths(100.0, 100.0),
                ..Style::DEFAULT
            })
            .unwrap();
        let green = tree
            .new_with_children(
                Style {
                    size: Size {
                        width: keyword,
                        height: Dimension::percent(1.0),
                    },
                    ..column(keyword)
                },
                &[leaf],
            )
            .unwrap();
        let outer = tree
            .new_with_children(
                Style {
                    size: Size::from_lengths(300.0, 200.0),
                    ..column(Dimension::length(300.0))
                },
                &[green],
            )
            .unwrap();
        tree.compute_layout(outer, Size::MAX_CONTENT).unwrap();
        assert_eq!(
            tree.layout(green).unwrap().size,
            Size {
                width: 100.0,
                height: 200.0
            }
        );
    }
}
