//! Сегменты правил промежутков сетки с учётом занятых ячеек.

use super::{Crossing, GAP_EPS, GapItem, GapRun};
use crate::paint::gap_rules::GapAxisRule;
use crate::paint::gap_rules::geometry::tracks::gaps_of;
use crate::paint::gap_rules::geometry::tracks::template_tracks;
use crate::paint::gap_rules::geometry::tracks::tracks_a;

pub(super) fn merge(mut v: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    v.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<(f32, f32)> = vec![];
    for (lo, hi) in v {
        match out.last_mut() {
            Some(last) if lo <= last.1 + GAP_EPS => last.1 = last.1.max(hi),
            _ => out.push((lo, hi)),
        }
    }
    out
}

pub(crate) fn subtract(parts: Vec<(f32, f32)>, (lo, hi): (f32, f32)) -> Vec<(f32, f32)> {
    let mut out = vec![];
    for (s, e) in parts {
        if hi <= s + GAP_EPS || lo >= e - GAP_EPS {
            out.push((s, e));
            continue;
        }
        if lo > s + GAP_EPS {
            out.push((s, lo));
        }
        if hi < e - GAP_EPS {
            out.push((hi, e));
        }
    }
    out
}

/// §visibility-items: заняты ли области по сторонам промежутка `[g0, g1]` в
/// пределах участка `[lo, hi]` вдоль линейки. Спан через промежуток занимает
/// обе стороны.
pub(super) fn occupied(
    items: &[GapItem],
    g0: f32,
    g1: f32,
    lo: f32,
    hi: f32,
    visibility: u8,
) -> bool {
    if visibility < 2 {
        return true;
    }
    let near = 2.0 * GAP_EPS;
    let before = items
        .iter()
        .any(|i| i.covers_b(lo, hi) && ((i.a1 - g0).abs() <= near || i.spans_a(g0, g1)));
    let after = items
        .iter()
        .any(|i| i.covers_b(lo, hi) && ((i.a0 - g1).abs() <= near || i.spans_a(g0, g1)));
    if visibility == 2 {
        before || after
    } else {
        before && after
    }
}

/// Решётка: линейки промежутков оси `a`. Пересекающие зазоры — промежутки
/// оси `b`; пересечение видимо (`breaks`), если хотя бы с одной стороны
/// поперечный зазор не перекрыт спаном (Blink: `kIntersection` идёт дальше
/// только при blocked-before И blocked-after); стык (`joins`) — если там есть
/// видимая поперечная линейка.
pub(crate) fn grid_runs(
    items: &[GapItem],
    gap_a: Option<f32>,
    gap_b: Option<f32>,
    rule: &GapAxisRule,
    cross: Option<&GapAxisRule>,
    tpl_a: Option<&[f32]>,
    tpl_b: Option<&[f32]>,
    abs: Option<(&[(f32, f32)], &[(f32, f32)])>,
) -> Vec<GapRun> {
    let flipped: Vec<GapItem> = items.iter().map(GapItem::flipped).collect();
    // Дорожки раскладки сильнее всего: это и есть коллекция дорожек сетки
    // (Blink `BuildGridTrackGapData`), с пустыми и схлопнутыми дорожками.
    // Годны, только если каждый элемент стоит краями на линиях дорожек: при
    // `rtl` и вертикальном письме раскладка отдаёт позиции в своей системе
    // отсчёта, и тогда остаётся прежний счёт по элементам и шаблону.
    let on_lines = |tracks: &[(f32, f32)], items: &[GapItem]| {
        items.iter().all(|i| {
            tracks.iter().any(|t| (t.0 - i.a0).abs() <= GAP_EPS)
                && tracks.iter().any(|t| (t.1 - i.a1).abs() <= GAP_EPS)
        })
    };
    if let Some((ta, tb)) = abs
        && !ta.is_empty()
        && !tb.is_empty()
        && on_lines(ta, items)
        && on_lines(tb, &flipped)
    {
        return grid_runs_on(items, &flipped, ta.to_vec(), tb.to_vec(), rule, cross);
    }
    // Дорожки шаблона сильнее выведенных из коробок (css-gaps-1 §gap-grid;
    // Blink `BuildGridTrackGapData` строит геометрию из коллекции дорожек).
    // Привязка не сошлась — остаётся прежний счёт по элементам, картинка не
    // меняется.
    let ta = tpl_a
        .and_then(|t| template_tracks(t, gap_a, items))
        .unwrap_or_else(|| tracks_a(items, gap_a));
    let tb = tpl_b
        .and_then(|t| template_tracks(t, gap_b, &flipped))
        .unwrap_or_else(|| tracks_a(&flipped, gap_b));
    grid_runs_on(items, &flipped, ta, tb, rule, cross)
}

pub(super) fn grid_runs_on(
    items: &[GapItem],
    flipped: &[GapItem],
    ta: Vec<(f32, f32)>,
    tb: Vec<(f32, f32)>,
    rule: &GapAxisRule,
    cross: Option<&GapAxisRule>,
) -> Vec<GapRun> {
    let ga = gaps_of(&ta);
    let gb = gaps_of(&tb);
    let r0 = tb.first().map_or(0.0, |t| t.0);
    let r1 = tb.last().map_or(0.0, |t| t.1);
    let n = ga.len();
    ga.iter()
        .enumerate()
        .map(|(k, &(g0, g1))| {
            let blocked = merge(
                items
                    .iter()
                    .filter(|i| i.spans_a(g0, g1))
                    .map(|i| (i.b0, i.b1))
                    .collect(),
            );
            let hidden = tb
                .iter()
                .copied()
                .filter(|&(lo, hi)| !occupied(items, g0, g1, lo, hi, rule.visibility))
                .collect();
            let sides = [ta.get(k).copied(), ta.get(k + 1).copied()];
            let crossings = gb
                .iter()
                .enumerate()
                .map(|(j, &(lo, hi))| {
                    let mut breaks = false;
                    let mut joins = false;
                    for side in sides.iter().flatten() {
                        let blocked_here = flipped
                            .iter()
                            .any(|i| i.spans_a(lo, hi) && i.covers_b(side.0, side.1));
                        if !blocked_here {
                            breaks = true;
                        }
                        if let Some(c) = cross
                            && !blocked_here
                            && c.styles.at(j, gb.len()).unwrap_or(false)
                            && occupied(flipped, lo, hi, side.0, side.1, c.visibility)
                        {
                            joins = true;
                        }
                    }
                    let cross_w = cross.and_then(|c| c.widths.at(j, gb.len())).unwrap_or(0.0);
                    Crossing {
                        lo,
                        hi,
                        breaks,
                        joins,
                        cross_w,
                    }
                })
                .collect();
            GapRun {
                g0,
                g1,
                r0,
                r1,
                crossings,
                blocked,
                hidden,
                start_edge: None,
                end_edge: None,
                index: k,
                count: n,
            }
        })
        .collect()
}
