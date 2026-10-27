//! Геометрия фрагментов при обычном и принудительном разрезании колонок.

use crate::layout::fragment::plan::{Space, fill_at};
use crate::layout::fragment::types::Kid;
use crate::layout::multicol::column_stack::ColumnStack;

fn kid(h: f32) -> Kid {
    Kid {
        h,
        over: h,
        ..Default::default()
    }
}

#[test]
fn slice_continues_content_in_successive_columns() {
    let (_, _, plan) = fill_at(&[kid(250.0)], &|_| 100.0, Space::columns(3));
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
fn box_that_fits_a_column_moves_before_last_resort_slicing() {
    let (_, _, plan) = fill_at(&[kid(75.), kid(25.)], &|_| 75.015625, Space::columns(2));
    assert_eq!(plan.len(), 2);
    assert_eq!((plan[1].col, plan[1].from, plan[1].h), (1, 0., 25.));
}

#[test]
fn class_a_boundaries_do_not_remove_class_c_breaks_in_overflow() {
    let mut block = kid(250.);
    block.over = 500.;
    block.cuts = vec![(125., 125.), (250., 250.)];
    let (_, _, plan) = fill_at(&[block], &|_| 100., Space::columns(5));
    assert_eq!(
        plan.iter().map(|f| (f.from, f.h)).collect::<Vec<_>>(),
        [
            (0., 100.),
            (100., 100.),
            (200., 100.),
            (300., 100.),
            (400., 100.)
        ]
    );
}

#[test]
fn row_line_preference_does_not_escape_avoid_between_measured_boundaries() {
    let mut block = kid(150.);
    block.cuts = vec![(25., 25.), (75., 75.), (100., 100.)];
    block.keep = vec![(0., 25.), (25., 75.), (75., 100.), (100., 150.)];
    block.solid = vec![(25., 75.05)];
    block.avoid = vec![(25., 75.05)];
    for height in [75.046875, 90., 100. - 1. / 64.] {
        let (columns, _, plan) = fill_at(&[block.clone()], &|_| height, Space::columns(3));
        assert_eq!(columns, 3, "height={height}");
        assert_eq!(plan[0].h, 25.);
        assert_eq!(plan[1].from, 25.);
        assert_eq!(plan[1].h, 75.);
    }
    let (columns, _, plan) = fill_at(&[block], &|_| 100., Space::columns(2));
    assert_eq!(columns, 2);
    assert_eq!((plan[0].h, plan[1].h), (100., 50.));
}

#[test]
fn forced_break_skips_margin_and_growth_uses_the_same_plan() {
    let mut k = kid(120.0);
    k.forced = vec![40.0];
    k.cuts = vec![(40.0, 50.0)];
    let kids = [k];
    let (_, _, plan) = fill_at(&kids, &|_| 100.0, Space::columns(2));
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
    let (_, _, plan) = fill_at(&[kid(40.0), block], &|_| 100.0, Space::columns(2));
    assert_eq!(plan.len(), 2);
    let f = plan[1];
    assert_eq!(
        (f.kid, f.copy, f.col, f.y, f.from, f.h),
        (1, 0, 1, 0.0, 0.0, 80.0)
    );
}

#[test]
fn flex_inside_avoid_moves_then_relaxes_and_keeps_forced_breaks() {
    let mut item = kid(120.0);
    item.keep = vec![(0.0, 120.0)];
    item.cuts = vec![(40.0, 40.0), (80.0, 80.0)];
    let (_, _, plan) = fill_at(&[kid(30.0), item.clone()], &|_| 100.0, Space::columns(4));
    // После переноса и ослабления avoid css-break-3 §4.4 сохраняет измеренные
    // точки: произвольный срез на 100px ещё не нужен. Все 120px продолжаются.
    assert_eq!(
        plan.iter()
            .map(|f| (f.kid, f.col, f.y, f.from, f.h))
            .collect::<Vec<_>>(),
        vec![
            (0, 0, 0.0, 0.0, 30.0),
            (1, 1, 0.0, 0.0, 80.0),
            (1, 2, 0.0, 80.0, 40.0)
        ]
    );
    item.forced = vec![40.0];
    let (_, _, plan) = fill_at(&[item], &|_| 100.0, Space::columns(4));
    assert_eq!((plan[0].col, plan[0].h), (0, 40.0));
    assert_eq!((plan[1].col, plan[1].from), (1, 40.0));
}
