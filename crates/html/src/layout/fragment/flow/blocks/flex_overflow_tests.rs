//! Переполнение flex-item параллельно следующему item (css-break-3 §2.1).
use super::{BlockBox, BlockKid, FlexBox, FlexFrame, FlexLine, Laid, Pushed};
use crate::layout::fragment::flow::FlowGeometry;

fn overflowing_column() -> (BlockBox, Laid) {
    let root = BlockBox {
        id: 10,
        flex: Some(FlexBox {
            items: vec![
                BlockKid {
                    id: 1,
                    inner: Some(BlockBox {
                        id: 1,
                        kids: vec![
                            BlockKid {
                                id: 3,
                                monolith: true,
                                ..Default::default()
                            },
                            BlockKid {
                                id: 4,
                                adjoins: true,
                                monolith: true,
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                BlockKid {
                    id: 2,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut laid = Laid::of_boxes(&[
        (10, 0.0, 200.0),
        (1, 0.0, 10.0),
        (2, 10.0, 200.0),
        (3, 0.0, 50.0),
        (4, 50.0, 150.0),
    ]);
    laid.flexes.insert(
        10,
        FlexFrame {
            column: true,
            lines: vec![FlexLine {
                start: 0.0,
                end: 200.0,
                items: vec![1, 2],
                order: vec![(0, Some(1)), (1, Some(2))],
                ..Default::default()
            }],
        },
    );
    (root, laid)
}

#[test]
fn column_overflow_does_not_constrain_the_following_item() {
    let (root, laid) = overflowing_column();
    let measured = root
        .breaks(200.0, &laid)
        .finish(FlowGeometry::default(), 200.0);
    assert_eq!(measured.solid, vec![(0.0, 10.0)]);
    assert_eq!(measured.cuts, vec![(10.0, 10.0)]);
    let kid = crate::layout::fragment::types::Kid {
        h: 200.0,
        over: 200.0,
        cuts: measured.cuts,
        solid: measured.solid,
        ..Default::default()
    };
    let (_, _, plan) = crate::layout::fragment::plan::fill_at(
        &[kid],
        &|_| 100.0,
        crate::layout::fragment::plan::Space::columns(4),
    );
    // css-break-3 §4.4: сначала используется точка между items; произвольный
    // срез оставшегося item допустим лишь в следующей пустой колонке.
    // Переполнение первого item не должно запрещать эту точку или терять контент.
    assert_eq!(
        plan.iter()
            .map(|f| (f.col, f.from, f.h))
            .collect::<Vec<_>>(),
        vec![(0, 0.0, 10.0), (1, 10.0, 100.0), (2, 110.0, 90.0)]
    );
}

#[test]
fn column_overflow_retains_its_own_fragmentation_target() {
    let (root, laid) = overflowing_column();
    let grows = crate::layout::fragment::push::flex_flow::growths(&root, &laid, 0.0, 100.0, 100.0);
    assert!(
        grows
            .iter()
            .any(|g| g.target == Pushed::Box(4) && g.amount == 50.0)
    );
    assert!(!grows.iter().any(|g| g.target == Pushed::Box(2)));
}

#[test]
fn forced_break_in_column_overflow_stays_in_its_own_flow() {
    let (mut root, laid) = overflowing_column();
    let inner = root.flex.as_mut().unwrap().items[0].inner.as_mut().unwrap();
    inner.inside_avoid = true;
    inner.kids[1].force_before = true;
    let measured = root
        .breaks(200.0, &laid)
        .finish(FlowGeometry::default(), 200.0);
    assert!(measured.forced.is_empty());
    assert_eq!(measured.keep, vec![(0.0, 10.0)]);
    let grows = crate::layout::fragment::push::flex_flow::growths(&root, &laid, 0.0, 100.0, 100.0);
    assert!(
        grows
            .iter()
            .any(|g| g.target == Pushed::Box(4) && g.amount == 50.0 && g.forced)
    );
    assert!(!grows.iter().any(|g| g.target == Pushed::Box(2)));
}

#[test]
fn overflowing_monolith_can_start_inside_its_allocated_item() {
    let (root, mut laid) = overflowing_column();
    laid.boxes.insert(1, (50.0, 60.0));
    laid.boxes.insert(2, (60.0, 250.0));
    laid.boxes.insert(3, (50.0, 100.0));
    laid.boxes.insert(4, (100.0, 200.0));
    let grows = crate::layout::fragment::push::flex_flow::growths(&root, &laid, 0.0, 80.0, 80.0);
    assert!(
        grows
            .iter()
            .any(|g| g.target == Pushed::Box(3) && g.amount == 30.0)
    );
    assert!(!grows.iter().any(|g| g.target == Pushed::Box(2)));
}
