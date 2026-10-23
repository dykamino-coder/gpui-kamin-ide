//! Согласование длинной цепочки forced breaks в независимом потоке flex.
use super::settle;
use crate::layout::fragment::flow::blocks::{
    BlockBox, BlockKid, FlexBox, FlexFrame, FlexLine, Laid, Pushed,
};
use crate::layout::fragment::plan::{self, Space};
use crate::layout::fragment::types::Kid;

#[test]
fn pending_fragment_corrections_require_a_fixed_point() {
    let mut pending = 9;
    let remaining = settle(|| {
        if pending > 0 {
            pending -= 1;
            (false, pending)
        } else {
            (true, pending)
        }
    });
    assert_eq!(remaining, 0, "a plan with pending corrections was accepted");
}

#[test]
fn all_forced_breaks_are_regrown_before_accepting_pages() {
    let blocks = BlockBox {
        id: 1,
        flex: Some(FlexBox {
            items: vec![BlockKid {
                id: 2,
                inner: Some(BlockBox {
                    id: 2,
                    kids: (3..=12)
                        .map(|id| BlockKid {
                            id,
                            monolith: true,
                            adjoins: id > 3,
                            force_before: id > 3,
                            ..Default::default()
                        })
                        .collect(),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut laid = Laid::of_boxes(&[(1, 0.0, 100.0), (2, 0.0, 100.0)]);
    for id in 3..=12 {
        let top = (id - 3) as f32 * 10.0;
        laid.boxes.insert(id, (top, top + 10.0));
    }
    laid.flexes.insert(
        1,
        FlexFrame {
            column: false,
            lines: vec![FlexLine {
                index: 0,
                start: 0.0,
                end: 100.0,
                items: vec![2],
            }],
        },
    );
    let mut passes = 0;
    settle(|| {
        let height = laid.boxes[&2].1;
        let kid = Kid {
            h: height,
            over: height,
            ..Default::default()
        };
        let (_, _, plan) = plan::fill(&[kid], 100.0, Space::pages(32));
        let growth = plan.iter().find_map(|f| {
            super::super::regrow::growths(&blocks, &Kid::default(), &laid, f, 100.0)
                .into_iter()
                .next()
        });
        if let Some(growth) = growth {
            let Pushed::Box(id) = growth.target else {
                panic!("expected block growth")
            };
            let top = laid.boxes[&id].0;
            for (&id, bounds) in &mut laid.boxes {
                if id <= 2 {
                    bounds.1 += growth.amount;
                } else if bounds.0 >= top {
                    bounds.0 += growth.amount;
                    bounds.1 += growth.amount;
                }
            }
            laid.flexes.get_mut(&1).unwrap().lines[0].end += growth.amount;
            passes += 1;
            return (false, ());
        }
        (true, ())
    });
    assert_eq!(passes, 9);
    for id in 4..=12 {
        assert_eq!(laid.boxes[&id].0, (id - 3) as f32 * 100.0);
    }
}

#[test]
fn named_page_height_is_used_before_accepting_the_plan() {
    let kids = [
        Kid {
            h: 100.0,
            over: 100.0,
            ..Default::default()
        },
        Kid {
            h: 300.0,
            over: 300.0,
            force_before: true,
            ..Default::default()
        },
    ];
    let mut names = vec![0];
    let mut passes = 0;
    let (pages, plan) = settle(|| {
        passes += 1;
        let tail = *names.last().unwrap();
        let height = |col| {
            if names.get(col).copied().unwrap_or(tail) == 0 {
                100.0
            } else {
                200.0
            }
        };
        let (pages, _, plan) = plan::fill_at(&kids, &height, Space::pages(16));
        let mut next = vec![None; pages];
        for f in &plan {
            next[f.col].get_or_insert(f.kid);
        }
        let mut last = 0;
        let next: Vec<_> = next
            .into_iter()
            .map(|name| {
                last = name.unwrap_or(last);
                last
            })
            .collect();
        let stable = names == next;
        names = next;
        (stable, (pages, plan))
    });
    assert_eq!(passes, 3);
    assert_eq!(pages, 3);
    assert_eq!(names, [0, 1, 1]);
    assert_eq!(
        plan.iter().map(|f| f.h).collect::<Vec<_>>(),
        [100.0, 200.0, 100.0]
    );
}
