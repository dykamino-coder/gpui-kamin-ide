//! Решатель распорок пагинации по разложенной геометрии кандидатов (`Geom`): первое
//! нарушение в порядке документа и сдвиг, который его чинит (`first_violation`), и усечение
//! `row-gap` у границы фрагментаинера (`gap_violation`). Координаты — копии (`0` — верх её
//! border-box). Ребёнок начинается на `y0` ниже верха первого фрагментаинера, поэтому
//! границы внутри копии — `k·H − y0` при `k ≥ 1` (css-break-3 §4: граница — точка разрыва
//! класса A потока). Blink принимает те же решения внутри раскладки: `MovePastBreakpoint` /
//! `BreakBeforeChildIfNeeded` (`fragmentation_utils.cc`) сдвигают монолит, не
//! влезающий в остаток, если он не в начале фрагментаинера, а принудительный разрыв всегда
//! начинает новый фрагментаинер.

use super::Cand;

use crate::tolerance::FRAGMENT_FIT_EPS;

/// Laid-out border box of a candidate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Geom {
    pub(crate) id: u64,
    pub(crate) cand: Cand,
    pub(crate) top: f32,
    pub(crate) bottom: f32,
}

/// First boundary strictly below `y` (copy coordinates).
fn next_boundary(y: f32, h: f32, y0: f32) -> f32 {
    let k = ((y + y0) / h).floor() + 1.0;
    k * h - y0
}

/// Whether `y` is the start of a fragmentainer (or the copy's own start at the
/// top of a fragmentainer).
fn at_start(y: f32, h: f32, y0: f32) -> bool {
    let r = (y + y0).rem_euclid(h);
    r < FRAGMENT_FIT_EPS || h - r < FRAGMENT_FIT_EPS
}

/// The first violated candidate in document order and the strut change that
/// fixes it: `(id, delta)`. `delta` may be negative when a strut overshot
/// (margins collapsing around a spacer).
pub(crate) fn first_violation(geoms: &[Geom], h: f32, y0: f32) -> Option<(u64, f32, f32)> {
    if h < 1.0 {
        return None;
    }
    for g in geoms {
        match g.cand {
            Cand::Forced { mt } => {
                let edge = g.top - mt;
                // The pushed box starts a fragmentainer; its preserved margin
                // follows the break.
                if edge > FRAGMENT_FIT_EPS && !at_start(edge, h, y0) {
                    let b = next_boundary(edge, h, y0);
                    return Some((g.id, b - edge, g.bottom - g.top));
                }
            }
            Cand::Item | Cand::Gap { .. } => {}
            Cand::Avoid { prev } => {
                if g.top < FRAGMENT_FIT_EPS || !at_start(g.top, h, y0) {
                    continue;
                }
                // Break before the previous box instead, unless that one
                // already starts its fragmentainer (no progress then).
                if let Some(p) = geoms.iter().find(|x| x.id == prev)
                    && p.top < g.top - FRAGMENT_FIT_EPS
                    && !at_start(p.top, h, y0)
                {
                    return Some((prev, g.top - p.top, p.bottom - p.top));
                }
            }
            Cand::Mono { soft } | Cand::Float { soft } => {
                let tall = g.bottom - g.top;
                if soft && tall > h + FRAGMENT_FIT_EPS && !matches!(g.cand, Cand::Float { .. }) {
                    continue;
                }
                if at_start(g.top, h, y0) {
                    continue;
                }
                let b = next_boundary(g.top, h, y0);
                if g.bottom > b + FRAGMENT_FIT_EPS {
                    return Some((g.id, b - g.top, tall));
                }
            }
        }
    }
    None
}

/// The first flex row whose preceding `row-gap` adjoins a fragmentainer
/// boundary: the row moves up to the boundary (`delta < 0`), the gap is
/// truncated. Returns the row's items, the move, the items of every row
/// after the first and the gap.
pub(crate) type GapMove = (Vec<u64>, f32, Vec<u64>, f32);

pub(crate) fn gap_violation(geoms: &[Geom], h: f32, y0: f32) -> Option<GapMove> {
    if h < 1.0 {
        return None;
    }
    let mut parents: Vec<u64> = Vec::new();
    for g in geoms {
        if let Cand::Gap { parent, .. } = g.cand
            && !parents.contains(&parent)
        {
            parents.push(parent);
        }
    }
    for parent in parents {
        let items: Vec<(u64, f32, f32, f32)> = geoms
            .iter()
            .filter_map(|g| match g.cand {
                Cand::Gap { gap, parent: p } if p == parent => Some((g.id, g.top, g.bottom, gap)),
                _ => None,
            })
            .collect();
        let mut tops: Vec<f32> = items.iter().map(|x| x.1).collect();
        tops.sort_by(f32::total_cmp);
        tops.dedup_by(|a, b| (*a - *b).abs() < FRAGMENT_FIT_EPS);
        let gap = items[0].3;
        for n in 1..tops.len() {
            let t = tops[n];
            // End of the previous row: its tallest item.
            let prev_end = items
                .iter()
                .filter(|x| (x.1 - tops[n - 1]).abs() < FRAGMENT_FIT_EPS)
                .map(|x| x.2)
                .fold(f32::MIN, f32::max);
            // Already truncated (or no full gap before the row).
            if t < prev_end + gap - FRAGMENT_FIT_EPS {
                continue;
            }
            // The first boundary at or after the start of the gap.
            let b = if at_start(prev_end, h, y0) {
                prev_end
            } else {
                next_boundary(prev_end, h, y0)
            };
            if b < t - FRAGMENT_FIT_EPS {
                let ids = items
                    .iter()
                    .filter(|x| (x.1 - t).abs() < FRAGMENT_FIT_EPS)
                    .map(|x| x.0)
                    .collect();
                let later = items
                    .iter()
                    .filter(|x| x.1 > tops[0] + FRAGMENT_FIT_EPS)
                    .map(|x| x.0)
                    .collect();
                return Some((ids, b - t, later, gap));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests;
