//! Проверки адресов независимых flex-потоков после новой раскладки.
use super::*;
use crate::layout::fragment::flow::ContentLine;
use crate::layout::fragment::flow::blocks::{FlexFrame, FlexLine};
#[test]
fn column_boundary_suppresses_gap_but_preserves_the_next_items_margin() {
    let mut laid = Laid::of_boxes(&[(1, 0., 100.), (2, 144., 200.)]);
    laid.margins.insert(2, (24., 0.));
    laid.flexes.insert(
        9,
        FlexFrame {
            column: true,
            lines: vec![FlexLine {
                items: vec![1, 2],
                ..Default::default()
            }],
        },
    );
    let token = Resume::at(&laid, 100.);
    assert_eq!(token.offset(&laid), Some(120.));
    laid.boxes.insert(2, (184., 240.));
    assert_eq!(token.offset(&laid), Some(160.));
}
#[test]
fn row_resumes_same_byte_after_width_changes_line_height_and_membership() {
    let mut old = Laid::of_boxes(&[(1, 0., 60.), (2, 0., 20.)]);
    old.flexes.insert(
        9,
        FlexFrame {
            column: false,
            lines: vec![FlexLine {
                start: 0.,
                end: 60.,
                items: vec![1, 2],
                ..Default::default()
            }],
        },
    );
    old.content.insert(
        1,
        vec![ContentLine {
            state: Default::default(),
            paragraph: 0,
            byte: 8,
            top: 20.,
            bottom: 40.,
            consumed_before: 0.0,
        }],
    );
    let token = Resume::at(&old, 20.);
    let mut new = Laid::of_boxes(&[(1, 0., 100.), (2, 100., 120.)]);
    new.flexes.insert(
        9,
        FlexFrame {
            column: false,
            lines: vec![
                FlexLine {
                    start: 0.,
                    end: 100.,
                    items: vec![1],
                    ..Default::default()
                },
                FlexLine {
                    start: 100.,
                    end: 120.,
                    items: vec![2],
                    ..Default::default()
                },
            ],
        },
    );
    new.content.insert(
        1,
        vec![ContentLine {
            state: Default::default(),
            paragraph: 0,
            byte: 8,
            top: 40.,
            bottom: 60.,
            consumed_before: 0.0,
        }],
    );
    assert_eq!(
        token.consumed(&new),
        HashMap::from([(1, (40., false)), (2, (20., true))])
    );
    assert_eq!(token.offset(&new), Some(0.));
}
#[test]
fn non_content_box_keeps_consumed_block_size_when_used_height_changes() {
    let old = Laid::of_boxes(&[(1, 0., 60.)]);
    let token = Resume {
        items: HashMap::from([(1, Address::BlockOffset(20.))]),
    };
    let mut new = old;
    new.boxes.insert(1, (0., 100.));
    assert_eq!(token.position(1, &new), Some(20.));
}
#[test]
fn unstarted_row_items_have_no_consumed_instruction() {
    let mut laid = Laid::of_boxes(&[(1, 0., 20.), (2, 20., 40.)]);
    laid.flexes.insert(
        9,
        FlexFrame {
            column: false,
            lines: vec![FlexLine {
                start: 0.,
                end: 40.,
                items: vec![1, 2],
                ..Default::default()
            }],
        },
    );
    assert_eq!(
        Resume::at(&laid, 20.).consumed(&laid),
        HashMap::from([(1, (20., true))])
    );
    assert!(Resume::at(&laid, 0.).consumed(&laid).is_empty());
}
#[test]
fn column_resumes_content_instead_of_old_y() {
    let mut old = Laid::of_boxes(&[(1, 0., 60.), (2, 60., 80.)]);
    old.flexes.insert(
        9,
        FlexFrame {
            column: true,
            lines: vec![FlexLine {
                items: vec![1, 2],
                ..Default::default()
            }],
        },
    );
    old.content.insert(
        1,
        vec![ContentLine {
            state: Default::default(),
            paragraph: 0,
            byte: 8,
            top: 20.,
            bottom: 40.,
            consumed_before: 0.0,
        }],
    );
    let token = Resume::at(&old, 20.);
    let mut new = old.clone();
    new.boxes.insert(1, (0., 100.));
    new.boxes.insert(2, (100., 120.));
    new.content.get_mut(&1).unwrap()[0].top = 40.;
    assert!(token.consumed(&new).is_empty());
    assert_eq!(token.offset(&new), Some(40.));
}

#[test]
fn column_keeps_physical_offset_inside_a_monolith_before_future_text() {
    let mut laid = Laid::of_boxes(&[(1, 0., 360.), (2, 0., 350.)]);
    laid.descendants.insert(1, vec![2]);
    laid.flexes.insert(
        9,
        FlexFrame {
            column: true,
            lines: vec![FlexLine {
                items: vec![1],
                ..Default::default()
            }],
        },
    );
    laid.content.insert(
        1,
        vec![ContentLine {
            top: 350.,
            bottom: 360.,
            ..Default::default()
        }],
    );
    for from in [100., 200., 300., 350.] {
        assert_eq!(Resume::at(&laid, from).offset(&laid), Some(from));
    }
    let physical = Resume::at(&laid, 300.);
    let text = Resume::at(&laid, 350.);
    laid.boxes.insert(1, (0., 380.));
    laid.boxes.insert(2, (0., 370.));
    laid.content.get_mut(&1).unwrap()[0].top = 370.;
    laid.content.get_mut(&1).unwrap()[0].bottom = 380.;
    assert_eq!(physical.offset(&laid), Some(300.));
    assert_eq!(text.offset(&laid), Some(370.));
}
