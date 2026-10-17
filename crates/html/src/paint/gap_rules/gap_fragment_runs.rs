//! Form fragment-local gap segments while retaining their stitched list indices.

use crate::layout::fragment::gap_fragment::Scope;
use crate::paint::gap_rules::geometry::GapRun;
use crate::tolerance::COORD_EPS;

pub(super) fn local_run(run: &GapRun, on_x: bool, scope: &Scope) -> Option<GapRun> {
    if !on_x {
        // CSS Gaps 1 §fragmentation: a gutter at a break disappears,
        // including a rule wider than that gutter or with negative insets.
        if run.g0 <= scope.start + COORD_EPS || run.g1 >= scope.cut - COORD_EPS {
            return None;
        }
        return Some(run.clone());
    }
    if run.r1 < scope.start - COORD_EPS || run.r0 > scope.cut + COORD_EPS {
        return None;
    }
    let mut local = run.clone();
    // Blink generates intersections per PhysicalBoxFragment, then resolves
    // cap/junction insets (gap_decorations_painter.cc:380-403). Clipping
    // an already inset whole-container rule cannot create a fragment cap.
    if local.r0 <= scope.start + COORD_EPS {
        local.r0 = scope.start;
        local.start_edge = None;
    }
    if local.r1 >= scope.cut - COORD_EPS {
        local.r1 = scope.end;
        local.end_edge = None;
    }
    local
        .crossings
        .retain(|c| c.lo > scope.start + COORD_EPS && c.hi < scope.cut - COORD_EPS);
    // A suppressed row gutter leaves an unused fragmentainer tail; the
    // adjoining track's visibility/spanning state continues through it.
    for intervals in [&mut local.hidden, &mut local.blocked] {
        for interval in intervals.iter_mut() {
            if interval.0 < scope.cut && interval.1 >= scope.cut - COORD_EPS {
                interval.1 = scope.end;
            }
        }
        intervals.retain(|&(s, e)| e > local.r0 && s < local.r1);
        for (s, e) in intervals.iter_mut() {
            *s = s.max(local.r0);
            *e = e.min(local.r1);
        }
    }
    Some(local)
}
