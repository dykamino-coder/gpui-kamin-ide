//! Идентичность floats, полосы исключений и координаты read-back внутри BFC.
use crate::prelude::*;
use crate::{BlockFormattingContext, Clear, FloatContext, FloatDirection, Point};

#[test]
fn negative_first_float_leaves_an_unbounded_empty_band_above_it() {
    let mut context = FloatContext::new();
    context.set_width(100.0);
    let position = context.place_floated_box(
        17,
        Size {
            width: 40.0,
            height: 30.0,
        },
        -20.0,
        [0.0; 2],
        FloatDirection::Left,
        Clear::None,
    );
    assert_eq!(position, Point { x: 0.0, y: -20.0 });
    let above = context.find_content_slot(-40.0, [0.0; 2], Clear::None, None);
    assert_eq!((above.x, above.y, above.width), (0.0, -40.0, 100.0));
    let beside = context.find_content_slot(-10.0, [0.0; 2], Clear::None, None);
    assert_eq!((beside.x, beside.y, beside.width), (40.0, -10.0, 60.0));
    assert_eq!(context.left_floats()[0].id, 17);
}

#[test]
fn tall_float_tail_remains_excluded_and_zero_height_does_not_create_a_band() {
    let mut context = FloatContext::new();
    context.set_width(100.0);
    context.place_floated_box(
        1,
        Size::ZERO,
        -50.0,
        [0.0; 2],
        FloatDirection::Left,
        Clear::None,
    );
    assert!(!context.has_active_floats(-100.0));
    context.place_floated_box(
        2,
        Size {
            width: 20.0,
            height: 20.0,
        },
        0.0,
        [0.0; 2],
        FloatDirection::Left,
        Clear::None,
    );
    context.place_floated_box(
        3,
        Size {
            width: 20.0,
            height: 60.0,
        },
        10.0,
        [0.0; 2],
        FloatDirection::Right,
        Clear::None,
    );
    let tail = context.find_content_slot(30.0, [0.0; 2], Clear::None, None);
    assert_eq!((tail.x, tail.y, tail.width), (0.0, 30.0, 80.0));
    assert_eq!(context.cleared_threshold(Clear::Both), Some(70.0));
    assert_eq!(context.right_floats()[0].id, 3);
}

#[test]
fn float_mark_reads_both_sides_in_the_callers_block_coordinates() {
    let mut bfc = BlockFormattingContext::new();
    let mut root = bfc.root_block_context();
    root.set_width(200.0);
    root.place_floated_box(
        1,
        Size {
            width: 10.0,
            height: 10.0,
        },
        0.0,
        FloatDirection::Left,
        Clear::None,
        false,
    );
    let mark = root.float_mark();
    let (left, right);
    {
        let mut nested = root.sub_context(20.0, [15.0, 25.0]);
        nested.apply_content_box_inset([5.0, 5.0]);
        left = nested.place_floated_box(
            2,
            Size {
                width: 20.0,
                height: 30.0,
            },
            0.0,
            FloatDirection::Left,
            Clear::None,
            false,
        );
        right = nested.place_floated_box(
            3,
            Size {
                width: 30.0,
                height: 30.0,
            },
            0.0,
            FloatDirection::Right,
            Clear::None,
            false,
        );
        assert_eq!(
            nested.floats_placed_since(mark).collect::<Vec<_>>(),
            vec![(2, left), (3, right)]
        );
    }
    let entries = root.floats_placed_since(mark).collect::<Vec<_>>();
    assert_eq!(
        entries,
        vec![
            (
                2,
                Point {
                    x: left.x + 15.0,
                    y: left.y + 20.0
                }
            ),
            (
                3,
                Point {
                    x: right.x + 15.0,
                    y: right.y + 20.0
                }
            ),
        ]
    );
    assert_eq!(root.floats_placed_since(root.float_mark()).count(), 0);
}
