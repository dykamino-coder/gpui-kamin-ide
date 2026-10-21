//! Пробеги линеек многоколонника из плана фрагментов: `column-rule` по коробкам колонок с
//! блокировкой под spanner'ами, `row-rule` по промежуткам между рядами колонок без spanner'а,
//! видимость по занятым колонкам соседних рядов. Геометрия идёт по коробкам колонок, а не по
//! границам элементов, как в гибком контейнере; Blink `ColumnGapAccumulator` так же держит
//! границы spanner'ов отдельно от промежутков рядов.
mod visibility;
use visibility::row_visible;

use super::{Crossing, GAP_RULE_EPS, GapItem, GapRun};
use crate::paint::gap_rules::{GapAxisRule, GapRuleSpec, MulticolGaps};

fn run(g0: f32, g1: f32, r0: f32, r1: f32, index: usize, count: usize) -> GapRun {
    GapRun {
        g0,
        g1,
        r0,
        r1,
        index,
        count,
        crossings: vec![],
        blocked: vec![],
        hidden: vec![],
        start_edge: None,
        end_edge: None,
    }
}
fn crossing(lo: f32, hi: f32, rule: Option<&GapAxisRule>, index: usize, count: usize) -> Crossing {
    let joins = true;
    Crossing {
        lo,
        hi,
        breaks: true,
        joins,
        cross_w: rule.map_or(0.0, |r| r.widths.at(index, count).unwrap_or(0.0)),
    }
}
fn visible(used: usize, column: usize, visibility: u8) -> bool {
    match visibility {
        1 => true,
        2 => column < used,
        _ => column + 1 < used,
    }
}
pub(crate) fn multicol_runs(data: &MulticolGaps, spec: &GapRuleSpec) -> (Vec<GapRun>, Vec<GapRun>) {
    let on_x = !spec.vertical;
    let columns: Vec<_> = data
        .columns
        .iter()
        .map(|b| GapItem::from_bounds(b, on_x))
        .collect();
    let mut rows: Vec<_> = data
        .rows
        .iter()
        .map(|(b, n)| (GapItem::from_bounds(b, on_x), *n))
        .collect();
    rows.sort_by(|a, b| a.0.b0.total_cmp(&b.0.b0));
    let spans: Vec<_> = data
        .spanners
        .iter()
        .map(|b| GapItem::from_bounds(b, on_x))
        .collect();
    let Some((first, _)) = rows.first() else {
        return (vec![], vec![]);
    };
    let last = rows.last().unwrap().0;
    let mut gaps = vec![];
    for pair in rows.windows(2) {
        let (lo, hi) = (pair[0].0.b1, pair[1].0.b0);
        if hi > lo + GAP_RULE_EPS
            && !spans
                .iter()
                .any(|s| s.b0 < hi - GAP_RULE_EPS && s.b1 > lo + GAP_RULE_EPS)
        {
            gaps.push((lo, hi, pair[0].1, pair[1].1));
        }
    }
    let mut col_runs = vec![];
    let count = columns.len().saturating_sub(1);
    if let Some(rule) = &spec.col {
        for (index, pair) in columns.windows(2).enumerate() {
            let (g0, g1) = if pair[0].a0 < pair[1].a0 {
                (pair[0].a1, pair[1].a0)
            } else {
                (pair[1].a1, pair[0].a0)
            };
            let reverse = if spec.vertical {
                spec.rev_y
            } else {
                spec.rev_x
            };
            let logical = if reverse { count - 1 - index } else { index };
            let mut col = run(g0, g1, first.b0, last.b1, logical, count);
            col.blocked = spans.iter().map(|s| (s.b0, s.b1)).collect();
            for (row, used) in &rows {
                if !visible(*used, index, rule.visibility) {
                    col.hidden.push((row.b0, row.b1));
                }
            }
            for (i, &(lo, hi, before, after)) in gaps.iter().enumerate() {
                let i = if spec.rev_x && spec.vertical {
                    gaps.len() - 1 - i
                } else {
                    i
                };
                let mut cross = crossing(lo, hi, spec.row.as_ref(), i, gaps.len());
                if matches!(rule.visibility, 0 | 3) {
                    let vis = spec.row.as_ref().map_or(0, |r| r.visibility);
                    cross.joins = row_visible(before, after, index, vis)
                        || row_visible(before, after, index + 1, vis);
                }
                col.crossings.push(cross);
            }
            col_runs.push(col);
        }
    }
    let mut row_runs = vec![];
    if let Some(rule) = &spec.row {
        for (index, &(g0, g1, before, after)) in gaps.iter().enumerate() {
            let logical = if spec.vertical && spec.rev_x {
                gaps.len() - 1 - index
            } else {
                index
            };
            let mut row = run(g0, g1, first.a0, first.a1, logical, gaps.len());
            for (i, col) in columns.iter().enumerate() {
                if !row_visible(before, after, i, rule.visibility) {
                    row.hidden.push((col.a0, col.a1));
                }
            }
            for (i, pair) in columns.windows(2).enumerate() {
                let (lo, hi) = if pair[0].a0 < pair[1].a0 {
                    (pair[0].a1, pair[1].a0)
                } else {
                    (pair[1].a1, pair[0].a0)
                };
                let mut cross = crossing(lo, hi, spec.col.as_ref(), i, count);
                if rule.visibility == 3 {
                    let vis = spec.col.as_ref().map_or(0, |r| r.visibility);
                    cross.joins = visible(before, i, vis) || visible(after, i, vis);
                }
                row.crossings.push(cross);
            }
            row_runs.push(row);
        }
    }
    (col_runs, row_runs)
}
