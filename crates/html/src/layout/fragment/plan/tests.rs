//! Планировщик: явный `Space` вместо скрытого состояния обхода.

use super::{Space, fill};
use crate::layout::fragment::types::Kid;

/// Монолит высоты `h` с верхним полем `mt` (поле первого ребёнка контекста не
/// усекается — курсор стоит ниже верха колонки).
fn monolith(h: f32, mt: f32) -> Kid {
    Kid {
        h,
        mt,
        monolith: true,
        over: h,
        ..Default::default()
    }
}

#[test]
fn monolith_overflows_a_fragmentainer_it_starts() {
    let (_, _, plan) = fill(&[monolith(80.0, 10.0)], 50.0, Space::columns(3));
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].col, 0);
}

#[test]
fn monolith_is_pushed_from_a_fragmentainer_not_at_its_start() {
    let space = Space::columns(3).with_not_at_start(1);
    let (_, _, plan) = fill(&[monolith(80.0, 10.0)], 50.0, space);
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].col, 1);
}

#[test]
fn page_and_column_spaces_differ_only_in_kind() {
    let (c, p) = (Space::columns(4), Space::pages(4));
    let parts = |s: Space| (s.limit, s.space.not_at_start, s.space.is_paginated());
    assert_eq!(parts(c), (4, 0, false));
    assert_eq!(parts(p), (4, 0, true));
}

#[test]
fn next_fragmentainer_replaces_height_and_resume_coordinate_before_cutting() {
    let kid = Kid {
        h: 120.,
        over: 120.,
        cuts: vec![(40., 40.), (80., 80.)],
        ..Default::default()
    };
    let mut calls = Vec::new();
    let (_, _, plan) = super::fill_measured(
        &[kid],
        &|_| 40.,
        Space::pages(6),
        &mut |_, copy, col, _, from| {
            calls.push((copy, col, from));
            (copy == 1).then(|| {
                (
                    Kid {
                        h: 100.,
                        over: 100.,
                        cuts: vec![(20., 20.), (60., 60.)],
                        ..Default::default()
                    },
                    20.,
                )
            })
        },
    );
    assert_eq!(
        plan.iter()
            .map(|f| (f.col, f.from, f.h))
            .collect::<Vec<_>>(),
        [(0, 0., 40.), (1, 20., 40.), (2, 60., 40.)]
    );
    assert_eq!(calls, [(0, 0, 0.), (1, 1, 40.), (2, 2, 60.)]);
}

fn slice(h: f32) -> Kid {
    Kid {
        h,
        over: h,
        ..Default::default()
    }
}

#[test]
fn exhausted_copy_caps_report_incomplete_and_keep_the_same_prefix() {
    for limit in [12, 48, 16] {
        for space in [Space::columns(limit), Space::pages(limit)] {
            let kid = slice((limit + 1) as f32 * 10.0);
            let (_, _, plan) = fill(std::slice::from_ref(&kid), 10.0, space);
            let (_, _, full) = fill(&[kid], 10.0, Space::columns(limit + 1));
            assert_eq!(plan.completion, super::Completion::Incomplete);
            assert_eq!(plan.len(), limit);
            for (a, b) in plan.iter().zip(full.iter()) {
                assert_eq!(
                    (a.copy, a.col, a.y, a.from, a.h),
                    (b.copy, b.col, b.y, b.from, b.h)
                );
            }
        }
    }
}

#[test]
fn exact_capacity_and_monolithic_overflow_are_complete() {
    for limit in [12, 48, 16] {
        let (_, _, plan) = fill(&[slice(limit as f32 * 10.0)], 10.0, Space::columns(limit));
        assert_eq!(plan.completion, super::Completion::Complete);
        let (_, _, plan) = fill(&[monolith(1000.0, 0.0)], 10.0, Space::columns(limit));
        assert_eq!(plan.completion, super::Completion::Complete);
    }
}

#[test]
fn forced_break_and_cloned_decoration_report_incomplete() {
    for kid in [
        Kid {
            forced: vec![10.0, 20.0],
            ..slice(30.0)
        },
        Kid {
            clone_dec: Some((1.0, 1.0)),
            ..slice(30.0)
        },
    ] {
        let (_, _, plan) = fill(std::slice::from_ref(&kid), 10.0, Space::columns(2));
        let (_, _, full) = fill(&[kid], 10.0, Space::columns(8));
        assert_eq!(plan.completion, super::Completion::Incomplete);
        assert_eq!(plan.len(), 2);
        assert_eq!((plan[0].copy, plan[1].copy), (0, 1));
        for (a, b) in plan.iter().zip(full.iter()) {
            assert_eq!((a.col, a.y, a.from, a.h), (b.col, b.y, b.from, b.h));
        }
    }
}

#[test]
fn missing_built_copy_marks_plan_without_dropping_fragments() {
    let (_, _, mut plan) = fill(&[slice(30.0)], 10.0, Space::columns(3));
    assert_eq!(plan.completion, super::Completion::Complete);
    plan.check_copies(|_| 2);
    assert_eq!(plan.completion, super::Completion::Incomplete);
    assert_eq!(plan.len(), 3);
    assert_eq!(plan[2].copy, 2);
    plan.check_copies(|_| 3);
    assert_eq!(plan.completion, super::Completion::Incomplete);

}
