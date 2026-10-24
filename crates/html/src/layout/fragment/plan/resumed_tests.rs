//! Интеграция token consumer с курсором соседей и переменной геометрией страниц.
use super::{Space, fill_resumed};
use crate::layout::fragment::flow::blocks::leaf::Leaf;
use crate::layout::fragment::token::Error;
use crate::layout::fragment::types::Kid;

#[test]
fn leaf_finishes_beyond_the_copy_budget_in_varying_fragmentainers() {
    let kids = [Kid {
        h: 2500.0,
        ..Kid::default()
    }];
    let (pages, _, plan) = fill_resumed(
        &kids,
        &|p| page(if p % 2 == 0 { 100.0 } else { 80.0 }),
        Space::pages(1),
        &|_, _| {
            Some(Leaf {
                content_block: 2500.0,
                ..Leaf::default()
            })
        },
        &mut |_| panic!("native leaf must not remeasure a copy"),
    )
    .unwrap();
    assert!(pages > 12);
    assert_eq!(pages, plan.len());
    let mut consumed = 0.0;
    for (index, fragment) in plan.iter().enumerate() {
        assert_eq!((fragment.copy, fragment.col), (index, index));
        assert_eq!(fragment.from, consumed);
        consumed += fragment.h;
    }
    assert_eq!(consumed, 2500.0);
}

#[test]
fn siblings_use_the_actual_end_of_the_resumed_leaf() {
    let kids = [40.0, 180.0, 20.0].map(|h| Kid {
        h,
        ..Kid::default()
    });
    let (_, _, plan) = fill_resumed(
        &kids,
        &|_| page(100.0),
        Space::pages(12),
        &|index, _| {
            (index == 1).then_some(Leaf {
                content_block: 180.0,
                ..Leaf::default()
            })
        },
        &mut |_| super::Measurement::default(),
    )
    .unwrap();
    let middle: Vec<_> = plan
        .iter()
        .filter(|f| f.kid == 1)
        .map(|f| (f.col, f.y, f.from, f.h))
        .collect();
    assert_eq!(
        middle,
        [
            (0, 40.0, 0.0, 60.0),
            (1, 0.0, 60.0, 100.0),
            (2, 0.0, 160.0, 20.0)
        ]
    );
    let last = plan.last().unwrap();
    assert_eq!((last.kid, last.col, last.y), (2, 2, 20.0));
}

#[test]
fn stalled_leaf_is_an_error_instead_of_a_partial_plan() {
    let result = fill_resumed(
        &[Kid {
            h: 1.0,
            ..Kid::default()
        }],
        &|_| page(0.0),
        Space::pages(12),
        &|_, _| {
            Some(Leaf {
                content_block: 1.0,
                ..Leaf::default()
            })
        },
        &mut |_| super::Measurement::default(),
    );
    assert!(matches!(result, Err(Error::NoProgress)));
}

#[test]
fn forced_break_preserves_the_following_margin_unforced_break_discards_it() {
    for forced in [false, true] {
        let kids = [
            Kid {
                h: 90.0,
                mb: 30.0,
                ..Kid::default()
            },
            Kid {
                h: 20.0,
                mt: 15.0,
                force_before: forced,
                ..Kid::default()
            },
        ];
        let (_, _, plan) = fill_resumed(
            &kids,
            &|_| page(100.0),
            Space::pages(1),
            &|index, _| {
                Some(Leaf {
                    content_block: kids[index].h,
                    ..Leaf::default()
                })
            },
            &mut |_| panic!("leaf does not remeasure"),
        )
        .unwrap();
        let last = plan.last().unwrap();
        // CSS Break 3 §5.2: before-break margin исчезает в обоих случаях.
        assert_eq!(
            (last.kid, last.col, last.y, last.h),
            (1, 1, if forced { 15.0 } else { 0.0 }, 20.0)
        );
    }
}

fn page(height: f32) -> crate::layout::fragment::token::Space {
    crate::layout::fragment::token::Space {
        block_size: Some(height),
        ..crate::layout::fragment::token::Space::pages()
    }
}
