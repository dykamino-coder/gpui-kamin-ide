//! Точки flex-фрагментации по линиям решателя и независимые продолжения элементов.
use super::{BlockBox, BlockKid, FlexBox, FlexFrame, FlexLine, FlexPush, Laid, Pushed};
use crate::layout::fragment::flow::FlowGeometry;

fn setup(column: bool) -> (BlockBox, Laid) {
    let items = (1..=3)
        .map(|id| BlockKid {
            id,
            monolith: true,
            ..Default::default()
        })
        .collect();
    let root = BlockBox {
        id: 10,
        flex: Some(FlexBox {
            items,
            main_gap: 0.0,
        }),
        ..Default::default()
    };
    let mut laid = Laid::of_boxes(&[(1, 0.0, 30.0), (2, 0.0, 50.0), (3, 60.0, 100.0)]);
    laid.flexes.insert(
        10,
        FlexFrame {
            column,
            lines: vec![
                FlexLine {
                    index: 0,
                    start: 0.0,
                    end: 50.0,
                    items: vec![1, 2],
                },
                FlexLine {
                    index: 1,
                    start: 60.0,
                    end: 100.0,
                    items: vec![3],
                },
            ],
        },
    );
    (root, laid)
}

#[test]
fn actual_line_membership_overrides_dom_sequence() {
    let (b, laid) = setup(false);
    let m = b
        .breaks(100.0, &laid)
        .finish(FlowGeometry::default(), 100.0);
    assert_eq!(m.cuts, vec![(50.0, 60.0)]);
    assert!(m.solid.contains(&(0.0, 50.0)));
    assert!(m.solid.contains(&(60.0, 100.0)));
    assert_eq!(
        b.pushed_at(60.0, &laid),
        Some(Pushed::Flex(FlexPush {
            container: 10,
            line: 0
        }))
    );
    assert_eq!(b.pushed_at(30.0, &laid), None);
}

#[test]
fn forced_line_boundary_wins_over_avoid() {
    let (mut b, laid) = setup(false);
    let items = &mut b.flex.as_mut().unwrap().items;
    items[0].avoid_after = true;
    items[2].force_before = true;
    let m = b
        .breaks(100.0, &laid)
        .finish(FlowGeometry::default(), 100.0);
    assert_eq!(m.forced, vec![60.0]);
    assert!(m.avoid.is_empty());
}

#[test]
fn avoid_line_boundary_does_not_make_previous_line_monolithic() {
    let (mut b, laid) = setup(false);
    b.flex.as_mut().unwrap().items[2].avoid_before = true;
    let m = b
        .breaks(100.0, &laid)
        .finish(FlowGeometry::default(), 100.0);
    assert_eq!(m.avoid.len(), 1);
    assert_eq!(m.avoid[0].0, 0.0);
    assert!(m.avoid[0].1 > 60.0);
    assert!(!m.solid.iter().any(|&(a, e)| a == 0.0 && e > 50.0));
}

#[test]
fn column_items_are_sequential_in_each_actual_column() {
    let (b, mut laid) = setup(true);
    laid.boxes.insert(2, (30.0, 80.0));
    laid.boxes.insert(3, (0.0, 40.0));
    let frame = laid.flexes.get_mut(&10).unwrap();
    frame.lines[0].items = vec![1, 2];
    frame.lines[1].items = vec![3];
    let mut flows = Vec::new();
    b.flex_flows(&laid, &mut flows);
    assert_eq!(flows.len(), 2);
    let m = flows[0]
        .breaks(80.0, &laid)
        .finish(FlowGeometry::default(), 80.0);
    assert_eq!(m.cuts, vec![(30.0, 30.0)]);
    assert!(m.solid.contains(&(30.0, 80.0)));
    assert_eq!(b.pushed_at(30.0, &laid), Some(Pushed::Box(2)));
}

#[test]
fn forced_break_inside_one_row_item_stays_in_its_parallel_flow() {
    let (mut b, mut laid) = setup(false);
    laid.boxes.insert(4, (0.0, 20.0));
    laid.boxes.insert(5, (20.0, 50.0));
    let item = &mut b.flex.as_mut().unwrap().items[1];
    item.monolith = false;
    item.inner = Some(BlockBox {
        id: 2,
        kids: vec![
            BlockKid {
                id: 4,
                monolith: true,
                ..Default::default()
            },
            BlockKid {
                id: 5,
                adjoins: true,
                force_before: true,
                monolith: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    });
    let m = b
        .breaks(100.0, &laid)
        .finish(FlowGeometry::default(), 100.0);
    assert!(m.forced.is_empty());
    let grows = crate::layout::fragment::push::flex_flow::growths(&b, &laid, 0.0, 40.0, 40.0);
    assert_eq!(grows.len(), 1);
    assert_eq!(
        (grows[0].at, grows[0].amount, grows[0].forced),
        (20.0, 20.0, true)
    );
    assert_eq!(grows[0].target, Pushed::Box(5));
}

#[test]
fn avoid_between_later_rows_closes_exact_boundary() {
    let (mut b, mut laid) = setup(false);
    let frame = laid.flexes.get_mut(&10).unwrap();
    frame.lines.insert(
        0,
        FlexLine {
            index: 2,
            start: 0.0,
            end: 20.0,
            items: vec![],
        },
    );
    frame.lines[1].start = 20.0;
    b.flex.as_mut().unwrap().items[2].avoid_before = true;
    let m = b
        .breaks(100.0, &laid)
        .finish(FlowGeometry::default(), 100.0);
    assert!(m.solid.iter().any(|&(a, e)| a == 20.0 && e > 60.01));
}

#[test]
fn forced_zero_height_rows_keep_distinct_growth_targets() {
    let (mut b, mut laid) = setup(false);
    for item in &mut b.flex.as_mut().unwrap().items {
        item.force_before = true;
    }
    laid.flexes.get_mut(&10).unwrap().lines = (0..3)
        .map(|index| FlexLine {
            index,
            start: 0.0,
            end: 0.0,
            items: vec![index as u64 + 1],
        })
        .collect();
    let grows = crate::layout::fragment::push::flex_flow::growths(&b, &laid, 0.0, 0.0, 100.0);
    assert_eq!(grows.len(), 2);
    assert_eq!(
        grows[0].target,
        Pushed::Flex(FlexPush {
            container: 10,
            line: 0
        })
    );
    assert_eq!(
        grows[1].target,
        Pushed::Flex(FlexPush {
            container: 10,
            line: 1
        })
    );
    assert!(grows.iter().all(|g| g.amount == 100.0 && g.forced));
}

#[test]
fn column_avoid_pushes_only_its_own_parallel_flow() {
    let (mut b, mut laid) = setup(true);
    laid.boxes.insert(1, (0.0, 80.0));
    laid.boxes.insert(2, (80.0, 150.0));
    laid.boxes.insert(3, (0.0, 100.0));
    let frame = laid.flexes.get_mut(&10).unwrap();
    frame.lines[0].items = vec![1, 2];
    frame.lines[1].items = vec![3];
    let item = &mut b.flex.as_mut().unwrap().items[1];
    item.monolith = false;
    item.inner = Some(BlockBox {
        id: 2,
        inside_avoid: true,
        ..Default::default()
    });
    let m = b
        .breaks(150.0, &laid)
        .finish(FlowGeometry::default(), 150.0);
    assert!(m.keep.is_empty());
    let grows = crate::layout::fragment::push::flex_flow::growths(&b, &laid, 0.0, 100.0, 100.0);
    assert_eq!(grows.len(), 1);
    assert_eq!(grows[0].target, Pushed::Box(2));
    assert_eq!(grows[0].amount, 20.0);
    assert!(!grows[0].forced);
}
