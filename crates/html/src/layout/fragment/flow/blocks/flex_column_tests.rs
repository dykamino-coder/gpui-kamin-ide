//! Ограничения последовательной flex-колонки и примыкающих декораций.
use super::{BlockBox, BlockKid, FlexBox, FlexFrame, FlexLine, Laid};
use crate::layout::fragment::flow::FlowGeometry;

#[test]
fn single_column_preserves_later_monolithic_ranges() {
    let blocks = BlockBox {
        id: 10,
        flex: Some(FlexBox {
            items: (1..=2)
                .map(|id| BlockKid {
                    id,
                    monolith: true,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut laid = Laid::of_boxes(&[(10, 0.0, 200.0), (1, 0.0, 100.0), (2, 100.0, 200.0)]);
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
    let measured = blocks
        .breaks(200.0, &laid)
        .finish(FlowGeometry::default(), 200.0);
    assert_eq!(measured.cuts, vec![(100.0, 100.0)]);
    assert!(measured.solid.contains(&(0.0, 100.0)));
    assert!(measured.solid.contains(&(100.0, 200.0)));
    let kid = crate::layout::fragment::types::Kid {
        h: 200.0,
        over: 200.0,
        cuts: measured.cuts,
        solid: measured.solid,
        overflow_top: true,
        ..Default::default()
    };
    let (_, _, plan) = crate::layout::fragment::plan::fill(
        &[kid],
        60.0,
        crate::layout::fragment::plan::Space::columns(4),
    );
    assert_eq!(plan.len(), 2);
    assert!(plan.iter().all(|f| f.h == 100.0));
}

#[test]
fn trailing_decoration_preserves_soft_avoid_in_single_column_and_blocks() {
    for flex in [false, true] {
        let items: Vec<_> = (1..=2)
            .map(|id| BlockKid {
                id,
                adjoins: true,
                inner: Some(BlockBox {
                    id,
                    inside_avoid: true,
                    ..Default::default()
                }),
                ..Default::default()
            })
            .collect();
        let mut root = BlockBox {
            id: 10,
            tail: 40.0,
            ..Default::default()
        };
        if flex {
            root.flex = Some(FlexBox {
                items,
                ..Default::default()
            });
        } else {
            root.kids = items;
        }
        let mut laid = Laid::of_boxes(&[(10, 0.0, 160.0), (1, 0.0, 60.0), (2, 60.0, 120.0)]);
        laid.flexes.insert(
            10,
            FlexFrame {
                column: true,
                lines: vec![FlexLine {
                    start: 0.0,
                    end: 120.0,
                    items: vec![1, 2],
                    order: vec![(0, Some(1)), (1, Some(2))],
                    ..Default::default()
                }],
            },
        );
        let measured = root
            .breaks(160.0, &laid)
            .finish(FlowGeometry::default(), 160.0);
        assert!(measured.keep.contains(&(60.0, 160.0)), "flex={flex}");
        assert!(!measured.solid.contains(&(60.0, 160.0)));
        let kid = crate::layout::fragment::types::Kid {
            h: 160.0,
            over: 160.0,
            cuts: measured.cuts,
            solid: measured.solid,
            keep: measured.keep,
            ..Default::default()
        };
        let starter = crate::layout::fragment::types::Kid {
            h: 40.0,
            over: 40.0,
            monolith: true,
            ..Default::default()
        };
        let (_, _, plan) = crate::layout::fragment::plan::fill(
            &[starter, kid],
            160.0,
            crate::layout::fragment::plan::Space::columns(4),
        );
        assert_eq!(
            plan.iter().map(|f| f.h).collect::<Vec<_>>(),
            vec![40.0, 60.0, 100.0]
        );
    }
}

#[test]
fn trailing_decoration_keeps_avoid_soft_and_respects_class_c_gap() {
    use crate::layout::fragment::{plan, types::Kid};
    for gap in [0.0, 10.0] {
        let end = 160.0 + gap;
        let root = BlockBox {
            id: 10,
            tail: 40.0,
            kids: vec![BlockKid {
                id: 1,
                inner: Some(BlockBox {
                    id: 1,
                    inside_avoid: true,
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        };
        let laid = Laid::of_boxes(&[(10, 0.0, end), (1, 0.0, 120.0)]);
        let measured = root.breaks(end, &laid).finish(FlowGeometry::default(), end);
        assert_eq!(
            measured.keep,
            vec![(0.0, if gap == 0.0 { end } else { 120.0 })]
        );
        assert!(!measured.solid.contains(&(0.0, end)));
        let kid = Kid {
            h: end,
            over: end,
            cuts: measured.cuts,
            solid: measured.solid,
            keep: measured.keep,
            ..Default::default()
        };
        let (_, _, pieces) = plan::fill(&[kid], 100.0, plan::Space::columns(4));
        assert_eq!(pieces.iter().map(|f| f.h).sum::<f32>(), end);
        assert!(pieces.iter().all(|f| f.h <= 100.0));
    }
}

#[test]
fn nested_trailing_decorations_extend_the_same_soft_keep() {
    let leaf = BlockBox {
        id: 1,
        inside_avoid: true,
        ..Default::default()
    };
    let middle = BlockBox {
        id: 2,
        tail: 20.0,
        kids: vec![BlockKid {
            id: 1,
            inner: Some(leaf),
            ..Default::default()
        }],
        ..Default::default()
    };
    let root = BlockBox {
        id: 10,
        tail: 20.0,
        kids: vec![BlockKid {
            id: 2,
            inner: Some(middle),
            ..Default::default()
        }],
        ..Default::default()
    };
    let laid = Laid::of_boxes(&[(10, 0.0, 160.0), (2, 0.0, 140.0), (1, 0.0, 120.0)]);
    let measured = root
        .breaks(160.0, &laid)
        .finish(FlowGeometry::default(), 160.0);
    assert_eq!(measured.keep, vec![(0.0, 160.0)]);
    assert!(!measured.solid.contains(&(0.0, 160.0)));
}
