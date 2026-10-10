//! Intrinsic grid dimensions accumulate fractional tracks without repeated f32 rounding.
use crate::prelude::*;
use crate::{GridPlacement, LengthPercentage, LengthPercentageAuto, Line, Rect, Size};

#[test]
fn spanning_contribution_keeps_its_total_in_both_axes() {
    for vertical in [false, true] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.disable_rounding();
        let placement = |start: i16, span: u16| Line {
            start: GridPlacement::Line(start.into()),
            end: GridPlacement::Span(span),
        };
        let mut children = Vec::new();
        for (start, extent) in [(3, 65.0), (4, 37.5)] {
            let mut style = Style {
                size: if vertical {
                    Size {
                        width: length(100.0),
                        height: length(extent),
                    }
                } else {
                    Size {
                        width: length(extent),
                        height: length(100.0),
                    }
                },
                ..Style::DEFAULT
            };
            if vertical {
                style.grid_row = placement(start, 1);
            } else {
                style.grid_column = placement(start, 1);
            }
            children.push(tree.new_leaf(style).unwrap());
        }
        let mut style = Style {
            min_size: Size {
                width: length(0.0),
                height: length(0.0),
            },
            size: if vertical {
                Size {
                    width: length(100.0),
                    height: auto(),
                }
            } else {
                Size {
                    width: auto(),
                    height: length(100.0),
                }
            },
            ..Style::DEFAULT
        };
        if vertical {
            style.grid_row = placement(3, 5);
            style.margin.top = LengthPercentageAuto::length(3.75);
            style.margin.bottom = LengthPercentageAuto::length(8.75);
        } else {
            style.grid_column = placement(3, 5);
            style.margin.left = LengthPercentageAuto::length(3.75);
            style.margin.right = LengthPercentageAuto::length(8.75);
        }
        children.push(tree.new_leaf_with_context(style, ()).unwrap());
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Grid,
                    size: if vertical {
                        Size {
                            width: length(980.0),
                            height: auto(),
                        }
                    } else {
                        Size {
                            width: auto(),
                            height: length(980.0),
                        }
                    },
                    grid_template_columns: if vertical {
                        vec![length(125.0)]
                    } else {
                        vec![auto(); 4]
                    },
                    grid_template_rows: if vertical {
                        vec![auto(); 4]
                    } else {
                        vec![length(125.0)]
                    },
                    border: Rect {
                        left: LengthPercentage::length(1.25),
                        right: LengthPercentage::length(1.25),
                        top: LengthPercentage::length(1.25),
                        bottom: LengthPercentage::length(1.25),
                    },
                    align_content: AlignContent::START,
                    justify_content: AlignContent::START,
                    ..Style::DEFAULT
                },
                &children,
            )
            .unwrap();
        tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, _, style| {
            crate::compute_leaf_layout(
                input,
                style,
                |_, _| 0.0,
                |known, available| {
                    let extent = match if vertical {
                        available.height
                    } else {
                        available.width
                    } {
                        AvailableSpace::MinContent => 155.0,
                        _ => 215.0,
                    };
                    if vertical {
                        Size {
                            width: known.width.unwrap_or(100.0),
                            height: known.height.unwrap_or(extent),
                        }
                    } else {
                        Size {
                            width: known.width.unwrap_or(extent),
                            height: known.height.unwrap_or(100.0),
                        }
                    }
                },
            )
        })
        .unwrap();
        let size = tree.layout(root).unwrap().size;
        assert_eq!(if vertical { size.height } else { size.width }, 230.0);
    }
}
