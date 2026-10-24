//! Запреты между элементами колонки не превращают пустые хвосты в монолиты.
use super::{BlockBox, BlockKid, FlexBox, FlexFrame, FlexLine, Laid, Pushed};
use crate::layout::fragment::flow::{FlowGeometry, LinePush};

fn column() -> (BlockBox, Laid) {
    let root = BlockBox {
        id: 10,
        head: 2.0,
        tail: 2.0,
        flex: Some(FlexBox {
            items: (1..=4)
                .map(|id| BlockKid {
                    id,
                    avoid_before: true,
                    inner: Some(BlockBox {
                        id,
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            main_gap: 10.0,
        }),
        ..Default::default()
    };
    let mut laid = Laid::of_boxes(&[(10, 0.0, 174.0)]);
    for id in 1..=4 {
        let top = 2.0 + (id - 1) as f32 * 40.0;
        laid.boxes.insert(id, (top, top + 30.0));
        laid.flows.insert(
            id,
            FlowGeometry {
                cuts: vec![(top + 18.0, top + 18.0)],
                solid: vec![(top, top + 18.0)],
                ..Default::default()
            },
        );
    }
    laid.flexes.insert(
        10,
        FlexFrame {
            column: true,
            lines: vec![FlexLine {
                start: 2.0,
                end: 152.0,
                items: vec![1, 2, 3, 4],
                order: (0..4).map(|i| (i, Some(i as u64 + 1))).collect(),
                ..Default::default()
            }],
        },
    );
    (root, laid)
}

#[test]
fn oversized_avoid_chain_keeps_item_boundaries_available() {
    let (root, laid) = column();
    let m = root
        .breaks(174.0, &laid)
        .finish(FlowGeometry::default(), 174.0);
    assert!(m.cuts.contains(&(72.0, 72.0)), "{m:?}");
    assert!(!m.solid.iter().any(|&(a, b)| a == 60.0 && b > 82.0));
    let growths = crate::layout::fragment::push::flex_flow::growths(&root, &laid, 0.0, 87.0, 90.0);
    assert_eq!(growths.len(), 1);
    assert_eq!(
        (growths[0].target.clone(), growths[0].amount),
        (Pushed::Box(3), 8.0)
    );
}

#[test]
fn later_avoid_chain_retreats_to_unrestricted_boundary() {
    let (mut root, laid) = column();
    root.flex.as_mut().unwrap().items[1].avoid_before = false;
    let m = root
        .breaks(174.0, &laid)
        .finish(FlowGeometry::default(), 174.0);
    assert!(m.avoid.iter().any(|&(a, b)| a == 42.0 && b > 122.0));
    let growths = crate::layout::fragment::push::flex_flow::growths(&root, &laid, 0.0, 87.0, 90.0);
    assert_eq!(growths.len(), 1);
    assert_eq!(
        (growths[0].target.clone(), growths[0].amount),
        (Pushed::Box(2), 48.0)
    );
}

#[test]
fn internal_continuation_remains_an_early_break() {
    let (root, mut laid) = column();
    laid.flows.get_mut(&2).unwrap().solid = vec![(42.0, 52.0), (52.0, 60.0)];
    laid.flows.get_mut(&2).unwrap().cuts = vec![(52.0, 52.0), (60.0, 60.0)];
    laid.lines.insert(
        2,
        vec![LinePush {
            at: 52.0,
            line: 1,
            growth: Default::default(),
        }],
    );
    let m = root
        .breaks(174.0, &laid)
        .finish(FlowGeometry::default(), 174.0);
    assert!(m.avoid.iter().any(|&(a, b)| a == 52.0 && b > 82.0));
}

#[test]
fn forced_boundary_ends_avoid_chain() {
    let (mut root, laid) = column();
    root.flex.as_mut().unwrap().items[2].force_before = true;
    let m = root
        .breaks(174.0, &laid)
        .finish(FlowGeometry::default(), 174.0);
    assert_eq!(m.forced, vec![72.0]);
    assert!(m.avoid.iter().any(|&(a, b)| a == 82.0 && b > 122.0));
}

#[test]
fn avoided_empty_item_boundary_moves_its_predecessor() {
    for after in [false, true] {
        let (mut root, mut laid) = column();
        root.head = 0.0;
        root.tail = 0.0;
        let flex = root.flex.as_mut().unwrap();
        flex.items.truncate(3);
        flex.main_gap = 0.0;
        for item in &mut flex.items {
            item.avoid_before = false;
        }
        flex.items[1].avoid_after = after;
        flex.items[2].avoid_before = !after;
        laid.flows.clear();
        laid.boxes
            .extend([(1, (0.0, 50.0)), (2, (50.0, 100.0)), (3, (100.0, 400.0))]);
        let line = &mut laid.flexes.get_mut(&10).unwrap().lines[0];
        line.start = 0.0;
        line.end = 400.0;
        line.items = vec![1, 2, 3];
        let grows = crate::layout::fragment::push::flex_flow::growths;
        let growth = grows(&root, &laid, 0.0, 100.0, 100.0);
        assert_eq!(growth.len(), 1, "break-after={after}");
        assert_eq!(growth[0].target, Pushed::Box(2));
        assert_eq!(growth[0].amount, 50.0);
        // Внутри пустого хвоста разрез допустим; avoid закрывает границу соседей.
        assert!(grows(&root, &laid, 0.0, 75.0, 75.0).is_empty());
        assert!(grows(&root, &laid, 50.0, 150.0, 150.0).is_empty());
    }
}
