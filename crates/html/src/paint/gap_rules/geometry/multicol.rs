//! Multicol gap geometry follows column boxes, not flex item boundaries.
//! Blink ColumnGapAccumulator keeps spanner boundaries separate from row gaps.
use super::{Crossing, GAP_EPS, GapItem, GapRun};
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
    let joins = rule.is_some_and(|r| r.styles.at(index, count).unwrap_or(false));
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
        if hi > lo + GAP_EPS
            && !spans
                .iter()
                .any(|s| s.b0 < hi - GAP_EPS && s.b1 > lo + GAP_EPS)
        {
            gaps.push((lo, hi));
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
            for (i, &(lo, hi)) in gaps.iter().enumerate() {
                let i = if spec.rev_x && spec.vertical {
                    gaps.len() - 1 - i
                } else {
                    i
                };
                col.crossings
                    .push(crossing(lo, hi, spec.row.as_ref(), i, gaps.len()));
            }
            col_runs.push(col);
        }
    }
    let mut row_runs = vec![];
    if spec.row.is_some() {
        for (index, &(g0, g1)) in gaps.iter().enumerate() {
            let logical = if spec.vertical && spec.rev_x {
                gaps.len() - 1 - index
            } else {
                index
            };
            let mut row = run(g0, g1, first.a0, first.a1, logical, gaps.len());
            for (i, pair) in columns.windows(2).enumerate() {
                let (lo, hi) = if pair[0].a0 < pair[1].a0 {
                    (pair[0].a1, pair[1].a0)
                } else {
                    (pair[1].a1, pair[0].a0)
                };
                row.crossings
                    .push(crossing(lo, hi, spec.col.as_ref(), i, count));
            }
            row_runs.push(row);
        }
    }
    (col_runs, row_runs)
}
