//! Intrinsic lane track constraints exclude the container's own padding and border.
use crate::style_helpers::*;
use crate::{
    BoxSizing, Display, GridLanes, LengthPercentage, LengthPercentageAuto, Rect, Size, Style,
    TaffyTree,
};

fn box_size(rows: bool, minimum: Option<f32>, maximum: Option<f32>, definite: Option<f32>) -> f32 {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let first = tree.new_leaf(Style::DEFAULT).unwrap();
    let second = tree.new_leaf(Style::DEFAULT).unwrap();
    let tracks = vec![minmax(length(10.0), fr(1.0)), minmax(length(10.0), fr(4.0))];
    let own = definite
        .map(crate::Dimension::length)
        .unwrap_or(crate::Dimension::auto());
    let min = minimum
        .map(LengthPercentageAuto::length)
        .unwrap_or(LengthPercentageAuto::auto());
    let max = maximum
        .map(LengthPercentageAuto::length)
        .unwrap_or(LengthPercentageAuto::auto());
    let padding = Rect {
        left: LengthPercentage::length(2.0),
        right: LengthPercentage::length(2.0),
        top: LengthPercentage::length(2.0),
        bottom: LengthPercentage::length(2.0),
    };
    let border = Rect {
        left: LengthPercentage::length(5.0),
        right: LengthPercentage::length(5.0),
        top: LengthPercentage::length(5.0),
        bottom: LengthPercentage::length(5.0),
    };
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                box_sizing: BoxSizing::ContentBox,
                grid_lanes: Some(GridLanes {
                    rows,
                    track_reverse: false,
                    fill_reverse: false,
                    dense: false,
                    tolerance: 0.0,
                    tolerance_pct: None,
                    stack_block: false,
                }),
                grid_template_columns: if rows { vec![] } else { tracks.clone() },
                grid_template_rows: if rows { tracks } else { vec![] },
                size: if rows {
                    Size {
                        width: crate::Dimension::length(50.0),
                        height: own,
                    }
                } else {
                    Size {
                        width: own,
                        height: crate::Dimension::length(50.0),
                    }
                },
                min_size: if rows {
                    Size {
                        width: LengthPercentageAuto::auto(),
                        height: min,
                    }
                } else {
                    Size {
                        width: min,
                        height: LengthPercentageAuto::auto(),
                    }
                },
                max_size: if rows {
                    Size {
                        width: LengthPercentageAuto::auto(),
                        height: max,
                    }
                } else {
                    Size {
                        width: max,
                        height: LengthPercentageAuto::auto(),
                    }
                },
                gap: Size {
                    width: LengthPercentage::length(33.0),
                    height: LengthPercentage::length(33.0),
                },
                padding,
                border,
                ..Style::DEFAULT
            },
            &[first, second],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let size = tree.layout(root).unwrap().size;
    if rows {
        size.height
    } else {
        size.width
    }
}

#[test]
fn minimum_flexible_track_axis_counts_own_edges_once() {
    for rows in [false, true] {
        assert_eq!(box_size(rows, Some(108.0), None, None), 122.0);
    }
}
#[test]
fn maximum_flexible_track_axis_keeps_content_box_constraint() {
    for rows in [false, true] {
        assert_eq!(box_size(rows, None, Some(70.0), None), 84.0);
    }
}
#[test]
fn definite_flexible_track_axis_keeps_content_box_constraint() {
    for rows in [false, true] {
        assert_eq!(box_size(rows, None, None, Some(70.0)), 84.0);
    }
}
