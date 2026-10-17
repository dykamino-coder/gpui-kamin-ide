//! Pagination struts from laid-out candidate geometry.
//!
//! Coordinates are the copy's own (`0` = its border-box top). The child starts
//! `y0` below the top of its first fragmentainer, so the fragmentainer
//! boundaries inside the copy are `k·H − y0` for `k ≥ 1` (css-break-3 §4: a
//! boundary is a class-A breakpoint of the flow thread). Blink takes the same
//! decisions inside layout: `MovePastBreakpoint` / `BreakBeforeChildIfNeeded`
//! (`fragmentation_utils.cc:896,1069`) push a monolith that does not fit the
//! remaining space unless it is at the fragmentainer start, and a forced break
//! always starts a new fragmentainer.

use super::Cand;

const EPS: f32 = 0.5;

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
    r < EPS || h - r < EPS
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
                if edge > EPS && !at_start(edge, h, y0) {
                    let b = next_boundary(edge, h, y0);
                    return Some((g.id, b - edge, g.bottom - g.top));
                }
            }
            Cand::Item | Cand::Gap { .. } => {}
            Cand::Mono { soft } => {
                let tall = g.bottom - g.top;
                if soft && tall > h + EPS {
                    continue;
                }
                if at_start(g.top, h, y0) {
                    continue;
                }
                let b = next_boundary(g.top, h, y0);
                if g.bottom > b + EPS {
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
        tops.dedup_by(|a, b| (*a - *b).abs() < EPS);
        let gap = items[0].3;
        for n in 1..tops.len() {
            let t = tops[n];
            // End of the previous row: its tallest item.
            let prev_end = items
                .iter()
                .filter(|x| (x.1 - tops[n - 1]).abs() < EPS)
                .map(|x| x.2)
                .fold(f32::MIN, f32::max);
            // Already truncated (or no full gap before the row).
            if t < prev_end + gap - EPS {
                continue;
            }
            // The first boundary at or after the start of the gap.
            let b = if at_start(prev_end, h, y0) {
                prev_end
            } else {
                next_boundary(prev_end, h, y0)
            };
            if b < t - EPS {
                let ids = items
                    .iter()
                    .filter(|x| (x.1 - t).abs() < EPS)
                    .map(|x| x.0)
                    .collect();
                let later = items
                    .iter()
                    .filter(|x| x.1 > tops[0] + EPS)
                    .map(|x| x.0)
                    .collect();
                return Some((ids, b - t, later, gap));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mono(id: u64, top: f32, bottom: f32) -> Geom {
        Geom {
            id,
            cand: Cand::Mono { soft: false },
            top,
            bottom,
        }
    }

    #[test]
    fn pushes_straddling_monolith_to_next_boundary() {
        let g = [mono(1, 50.0, 150.0)];
        assert_eq!(first_violation(&g, 100.0, 0.0).map(|v| v.1), Some(50.0));
        // Starting 20px down its first column, the boundary is at 80.
        assert_eq!(first_violation(&g, 100.0, 20.0).map(|v| v.1), Some(30.0));
    }

    #[test]
    fn keeps_monolith_at_fragmentainer_start() {
        let g = [mono(1, 100.0, 250.0)];
        assert!(first_violation(&g, 100.0, 0.0).is_none());
    }

    #[test]
    fn forced_break_moves_border_edge_and_keeps_margin() {
        let g = [Geom {
            id: 2,
            cand: Cand::Forced { mt: 10.0 },
            top: 70.0,
            bottom: 90.0,
        }];
        assert_eq!(first_violation(&g, 100.0, 0.0).map(|v| v.1), Some(40.0));
        let done = [Geom {
            top: 110.0,
            bottom: 130.0,
            ..g[0]
        }];
        assert!(first_violation(&done, 100.0, 0.0).is_none());
    }
}
