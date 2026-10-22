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
