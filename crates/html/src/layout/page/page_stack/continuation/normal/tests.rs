//! Адреса продолжений сохраняют владельца после повторной раскладки.
use super::*;
use crate::layout::fragment::flow::ContentLine;
use crate::layout::fragment::flow::blocks::{BlockKid, FlexBox, GridBox};

#[test]
fn sibling_boundary_tracks_the_child_after_relayout() {
    let blocks = BlockBox {
        id: 1,
        kids: vec![
            BlockKid {
                id: 2,
                ..Default::default()
            },
            BlockKid {
                id: 3,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let old = Laid::of_boxes(&[(1, 0., 288.), (2, 0., 96.), (3, 96., 288.)]);
    let cursor = Cursor::at(&blocks, &old, &[], 96.).unwrap();
    let new = Laid::of_boxes(&[(1, 0., 320.), (2, 0., 128.), (3, 128., 320.)]);
    assert_eq!(cursor.offset(&new, &[]), Some(128.));
}

#[test]
fn empty_remainder_keeps_consumed_size_instead_of_jumping_to_the_next_child() {
    let blocks = BlockBox {
        id: 1,
        kids: vec![
            BlockKid {
                id: 2,
                ..Default::default()
            },
            BlockKid {
                id: 3,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let old = Laid::of_boxes(&[(1, 0., 300.), (2, 0., 200.), (3, 200., 300.)]);
    let cursor = Cursor::at(&blocks, &old, &[], 96.).unwrap();
    let new = Laid::of_boxes(&[(1, 0., 400.), (2, 0., 300.), (3, 300., 400.)]);
    assert_eq!(cursor.offset(&new, &[]), Some(96.));
}

#[test]
fn definite_height_continues_after_its_last_line() {
    let blocks = BlockBox {
        id: 1,
        ..Default::default()
    };
    let old = Laid::of_boxes(&[(1, 0., 600.)]);
    let inline = vec![Inline {
        lines: vec![ContentLine {
            top: 0.,
            bottom: 20.,
            ..Default::default()
        }],
        ..Default::default()
    }];
    let cursor = Cursor::at(&blocks, &old, &inline, 200.).unwrap();
    assert_eq!(cursor.offset(&old, &inline), Some(200.));
}

#[test]
fn normal_line_uses_content_address_after_rewrap() {
    let blocks = BlockBox {
        id: 1,
        ..Default::default()
    };
    let old = Laid::of_boxes(&[(1, 0., 100.)]);
    let mut line = ContentLine {
        paragraph: 2,
        top: 20.,
        bottom: 40.,
        ..Default::default()
    };
    line.state.text_pos = 12;
    let mut inline = vec![Inline {
        lines: vec![line],
        ..Default::default()
    }];
    let cursor = Cursor::at(&blocks, &old, &inline, 20.).unwrap();
    inline[0].lines[0].top = 60.;
    assert_eq!(cursor.offset(&old, &inline), Some(60.));
}

#[test]
fn nested_flex_keeps_its_independent_item_cursors() {
    let blocks = BlockBox {
        id: 1,
        kids: vec![BlockKid {
            id: 2,
            inner: Some(BlockBox {
                id: 2,
                flex: Some(FlexBox::default()),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    };
    let old = Laid::of_boxes(&[(1, 0., 100.), (2, 0., 100.)]);
    assert!(Cursor::at(&blocks, &old, &[], 50.).is_none());
}

#[test]
fn grid_row_boundary_tracks_all_remaining_items_after_relayout() {
    let mut grid = GridBox::default();
    grid.items.resize_with(3, Default::default);
    for (item, id) in grid.items.iter_mut().zip([2, 3, 4]) {
        item.id = id;
    }
    let blocks = BlockBox {
        id: 1,
        grid: Some(grid),
        ..Default::default()
    };
    let old = Laid::of_boxes(&[
        (1, 0., 336.),
        (2, 0., 144.),
        (3, 192., 336.),
        (4, 192., 336.),
    ]);
    let cursor = Cursor::at(&blocks, &old, &[], 192.).unwrap();
    let new = Laid::of_boxes(&[
        (1, 0., 288.),
        (2, 0., 144.),
        (3, 156., 288.),
        (4, 144., 288.),
    ]);
    assert_eq!(cursor.offset(&new, &[]), Some(144.));
}
