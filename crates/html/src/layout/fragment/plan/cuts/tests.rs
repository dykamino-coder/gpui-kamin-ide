//! Геометрия фрагментов при обычном и принудительном разрезании колонок.

use crate::layout::fragment::plan::{Space, fill_at};
use crate::layout::fragment::types::{Kid, RowKeep};
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
fn forced_break_skips_margin_and_growth_uses_the_same_plan() {
    let mut k = kid(120.0);
    k.forced = vec![40.0];
    k.cuts = vec![(40.0, 50.0)];
    let kids = [k];
    let (_, _, plan) = fill_at(&kids, &|_| 100.0, Space::columns(2));
    assert_eq!(plan.len(), 2);
    assert_eq!((plan[0].from, plan[0].h), (0.0, 40.0));
    assert_eq!((plan[1].from, plan[1].h), (50.0, 70.0));
    let mut stack = ColumnStack::new(
        Vec::new(),
        2,
        0.0,
        Some(100.0),
        None,
        None,
        None,
        Default::default(),
    );
    stack.copies = 2;
    assert_eq!(stack.grow_points(&kids), vec![(0, 50.0, 60.0, true)]);
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
    assert_eq!((plan[1].col, plan[1].y, plan[1].from), (1, 0.0, 0.0));
    assert_eq!(plan[1].h, 100.0);
    item.forced = vec![40.0];
    let (_, _, plan) = fill_at(&[item], &|_| 100.0, Space::columns(4));
    assert_eq!((plan[0].col, plan[0].h), (0, 40.0));
    assert_eq!((plan[1].col, plan[1].from), (1, 40.0));
}

/// Таблица из трёх рядов по 40 с зазором 2 (css-tables-3 §breaking-rules).
fn table_rows() -> Kid {
    let mut t = kid(124.0);
    t.cuts = vec![(40.0, 42.0), (82.0, 84.0)];
    let row = |prev, open, start| RowKeep {
        prev,
        open,
        start,
        end: start + 40.0,
    };
    t.table_rows = vec![row(0.0, 40.0, 42.0), row(42.0, 82.0, 84.0)];
    t
}

fn pieces(k: Kid, block: f32, inline: f32) -> Vec<(f32, f32)> {
    let space = Space::columns(4).with_inline_size(inline);
    let (_, _, plan) = fill_at(&[k], &|_| block, space);
    plan.iter().map(|f| (f.from, f.h)).collect()
}

#[test]
fn a_row_at_most_half_the_fragmentainer_moves_whole_to_the_next_one() {
    // Ряд 84..124 не влез в колонку 100 и не выше половины 100×200: разрыв перед
    // ним, зазор усечён.
    assert_eq!(
        pieces(table_rows(), 100.0, 200.0),
        [(0.0, 82.0), (84.0, 40.0)]
    );
}

#[test]
fn freely_fragmentable_rows_still_break_at_the_edge() {
    // Колонка уже двух высот ряда: ряд freely fragmentable, срез по краю.
    assert_eq!(
        pieces(table_rows(), 100.0, 70.0),
        [(0.0, 100.0), (100.0, 24.0)]
    );
    // Ряд 152..172 малый, но предыдущий ряд — продолжение с прошлой колонки: ни один
    // ряд не влез во фрагмент целиком, и малый ряд режется краем.
    let mut tall = kid(172.0);
    tall.cuts = vec![(150.0, 152.0)];
    tall.table_rows = vec![RowKeep {
        prev: 0.0,
        open: 150.0,
        start: 152.0,
        end: 172.0,
    }];
    assert_eq!(
        pieces(tall, 80.0, 200.0),
        [(0.0, 80.0), (80.0, 80.0), (160.0, 12.0)]
    );
}
