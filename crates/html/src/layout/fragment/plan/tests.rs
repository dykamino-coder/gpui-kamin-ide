//! Планировщик: явный `Space` вместо скрытого состояния обхода.

use super::{Space, fill};
use crate::layout::fragment::types::{Kid, Par, RepeatGeom};

/// Монолит высоты `h` с верхним полем `mt` (поле первого ребёнка контекста не
/// усекается — курсор стоит ниже верха колонки).
fn monolith(h: f32, mt: f32) -> Kid {
    Kid {
        h,
        mt,
        mb: 0.0,
        monolith: true,
        cuts: Vec::new(),
        force_before: false,
        force_after: false,
        avoid_before: false,
        avoid_after: false,
        forced: Vec::new(),
        solid: Vec::new(),
        span: false,
        over: h,
        clone_dec: None,
        overflow_top: false,
        repeat: RepeatGeom::default(),
        par: Par::default(),
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
    let space = Space {
        not_at_start: 1,
        ..Space::columns(3)
    };
    let (_, _, plan) = fill(&[monolith(80.0, 10.0)], 50.0, space);
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].col, 1);
}

#[test]
fn page_and_column_spaces_differ_only_in_kind() {
    let (c, p) = (Space::columns(4), Space::pages(4));
    assert_eq!((c.limit, c.not_at_start, c.paged), (4, 0, false));
    assert_eq!((p.limit, p.not_at_start, p.paged), (4, 0, true));
}
