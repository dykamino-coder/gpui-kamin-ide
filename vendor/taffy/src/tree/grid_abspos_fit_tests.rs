//! Absolute grid children and non-stretched items use the same fit-content width.
//! A wrapped paragraph can report a shorter longest line than its available width.

use crate::style_helpers::*;
use crate::{
    AlignItems, AvailableSpace, Direction, Display, GridPlacement, Line, Position, Size, Style,
    TaffyTree,
};

mod cross_axis;
mod margin_space;

fn width(position: Position, direction: Direction) -> f32 {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let placement = Line {
        start: GridPlacement::Line(1.into()),
        end: GridPlacement::Line(2.into()),
    };
    let child = tree
        .new_leaf_with_context(
            Style {
                position,
                grid_column: placement.clone(),
                grid_row: placement,
                justify_self: Some(AlignItems::START),
                align_self: Some(AlignItems::START),
                ..Style::DEFAULT
            },
            (),
        )
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                direction,
                size: Size {
                    width: length(200.0),
                    height: length(100.0),
                },
                grid_template_columns: vec![length(100.0)],
                grid_template_rows: vec![length(25.0)],
                ..Style::DEFAULT
            },
            &[child],
        )
        .unwrap();
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, _, style| {
        crate::compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |known, available| Size {
                width: known.width.unwrap_or(match available.width {
                    AvailableSpace::MinContent => 30.0,
                    AvailableSpace::MaxContent => 140.0,
                    // Model wrapping: the longest resulting line occupies 80 of 100.
                    AvailableSpace::Definite(value) => value.min(80.0),
                }),
                height: known.height.unwrap_or(20.0),
            },
        )
    })
    .unwrap();
    tree.layout(child).unwrap().size.width
}

#[test]
fn automatic_absolute_width_matches_fit_content_in_both_directions() {
    for direction in [Direction::Ltr, Direction::Rtl] {
        assert_eq!(width(Position::Relative, direction), 100.0);
        assert_eq!(width(Position::Absolute, direction), 100.0);
    }
}
