//! Геометрия фрагментов при обычном и принудительном разрезании колонок.

use crate::layout::fragment::plan::{Space, fill};
use crate::layout::fragment::types::Kid;
use crate::layout::multicol::column_stack::ColumnStack;

fn kid(h: f32) -> Kid {
    Kid {
        h,
        mt: 0.0,
        mb: 0.0,
        monolith: false,
        cuts: Vec::new(),
        force_before: false,
        force_after: false,
        avoid_before: false,
        avoid_after: false,
        forced: Vec::new(),
        solid: Vec::new(),
        avoid: Vec::new(),
        span: false,
        over: h,
        clone_dec: None,
        overflow_top: false,
        repeat: Default::default(),
        par: Default::default(),
    }
}

#[test]
fn slice_continues_content_in_successive_columns() {
    let (_, _, plan) = fill(&[kid(250.0)], 100.0, Space::columns(3));
    let geometry: Vec<_> = plan
        .iter()
        .map(|f| (f.kid, f.copy, f.col, f.y, f.from, f.h, f.head, f.foot))
        .collect();
    assert_eq!(
        geometry,
        [
            (0, 0, 0, 0.0, 0.0, 100.0, 0.0, 0.0),
            (0, 1, 1, 0.0, 100.0, 100.0, 0.0, 0.0),
            (0, 2, 2, 0.0, 200.0, 50.0, 0.0, 0.0),
        ]
    );
}

#[test]
fn forced_break_skips_margin_and_growth_uses_the_same_plan() {
    let mut k = kid(120.0);
    k.forced = vec![40.0];
    k.cuts = vec![(40.0, 50.0)];
    let kids = [k];
    let (_, _, plan) = fill(&kids, 100.0, Space::columns(2));
    assert_eq!(plan.len(), 2);
    assert_eq!((plan[0].from, plan[0].h), (0.0, 40.0));
    assert_eq!((plan[1].from, plan[1].h), (50.0, 70.0));
    assert_eq!(
        ColumnStack::frags_of(&kids, 2, Some(100.0), None, 2),
        vec![vec![(0.0, 40.0), (50.0, 70.0)]]
    );
    assert_eq!(
        ColumnStack::growths(&kids, 2, Some(100.0), None, 2),
        vec![(0, 50.0, 60.0, true)]
    );
}

#[test]
fn monolith_moves_to_empty_column_without_leaking_previous_cursor() {
    let mut block = kid(80.0);
    block.monolith = true;
    let (_, _, plan) = fill(&[kid(40.0), block], 100.0, Space::columns(2));
    assert_eq!(plan.len(), 2);
    let f = plan[1];
    assert_eq!(
        (f.kid, f.copy, f.col, f.y, f.from, f.h),
        (1, 0, 1, 0.0, 0.0, 80.0)
    );
}
