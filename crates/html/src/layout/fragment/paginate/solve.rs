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
