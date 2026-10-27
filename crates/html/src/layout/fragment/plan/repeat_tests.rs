//! Укладка таблицы с повторяемой шапкой: шапка только у продолжений, правило четверти на
//! каждой странице, конечность последовательности фрагментов.

use super::{Completion, Space, fill_at};
use crate::layout::fragment::types::{Kid, RepeatGeom};

/// Таблица: шапка 0..20 (`break-inside: avoid`), шесть рядов по 40 до 260.
fn table() -> Kid {
    let ends = [20.0, 60.0, 100.0, 140.0, 180.0, 220.0];
    Kid {
        h: 260.0,
        over: 260.0,
        cuts: ends.iter().map(|&e| (e, e)).collect(),
        solid: vec![(0.0, 20.0)],
        repeat: RepeatGeom {
            head: 20.0,
            head_end: 20.0,
            foot_at: f32::MAX,
            foot_end: f32::MAX,
            box_top: 0.0,
            box_end: 260.0,
            head_need: 80.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// `(copy, head, from, h)` фрагментов плана.
fn pieces(
    target: &dyn Fn(usize) -> f32,
    space: Space,
) -> (Completion, Vec<(usize, f32, f32, f32)>) {
    let (_, _, plan) = fill_at(&[table()], target, space);
    let out = plan.iter().map(|f| (f.copy, f.head, f.from, f.h)).collect();
    (plan.completion, out)
}

/// Каждый фрагмент продвигает тело таблицы, и всё оно уложено ровно один раз.
fn assert_progress(frags: &[(usize, f32, f32, f32)]) {
    let mut end = 0.0;
    for &(_, _, from, h) in frags {
        assert!(h > 0.0, "{frags:?}");
        assert!((from - end).abs() < 0.01, "{frags:?}");
        end = from + h;
    }
    assert!((end - 260.0).abs() < 0.01, "{frags:?}");
}

#[test]
fn header_repeats_only_on_continuations_past_the_original() {
    let (done, frags) = pieces(&|_| 100.0, Space::columns(8));
    assert_eq!(done, Completion::Complete);
    assert_progress(&frags);
    assert_eq!(
        frags[0].1, 0.0,
        "the original header is in the first fragment"
    );
    assert!(frags[1..].iter().all(|f| f.1 == 20.0 && f.2 >= 20.0));
}

#[test]
fn a_page_below_four_header_heights_does_not_repeat_it() {
    // Страница 60 — четверть 15 меньше шапки 20: на ней повтора нет; в
    // колонках той же высоты шапка повторяется (решение принято в первой).
    let heights = |c: usize| if c == 0 { 100.0 } else { 60.0 };
    let (done, paged) = pieces(&heights, Space::pages(8));
    assert_eq!(done, Completion::Complete);
    assert_progress(&paged);
    assert!(paged[1..].iter().all(|f| f.1 == 0.0), "{paged:?}");
    let (_, columns) = pieces(&heights, Space::columns(8));
    assert_progress(&columns);
    assert!(columns[1..].iter().all(|f| f.1 == 20.0), "{columns:?}");
}

#[test]
fn fragmentainers_smaller_than_the_bands_still_advance_the_table() {
    // Без допуска полос шапка 20 в колонке 15 съедала бы всё место, и
    // фрагменты из одних повторов шли бы до исчерпания копий.
    let heights = |c: usize| if c == 0 { 100.0 } else { 15.0 };
    for space in [Space::columns(64), Space::pages(64)] {
        let (done, frags) = pieces(&heights, space);
        assert_eq!(done, Completion::Complete);
        assert_progress(&frags);
        assert!(frags[1..].iter().all(|f| f.1 == 0.0), "{frags:?}");
    }
}
