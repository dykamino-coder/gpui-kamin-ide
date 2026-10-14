//! css-gaps-1 §visibility-items for the grid-axis (main) gaps of grid lanes.
//!
//! A main gap between lanes `k` and `k + 1` is cut into segments at every
//! cross gap of either lane (Blink `GridLanesMainGapSegmentWalker::Next`:
//! segment ends are the cross-gap centres of both adjacent lanes). A side of
//! a segment is occupied when an item placed in that lane (a spanner counts
//! in every lane it covers) overlaps the segment along the stacking axis; a
//! lane that ends early leaves the segments past its last item empty
//! (`column-gap-decorations-rule-visibility-items-between-005`). Stacking
//! (cross) gaps are never filtered: the property only applies to grid-axis
//! rules in grid lanes.

use super::{GAP_EPS, GapItem};

/// Segments `[lo, hi]` of the main gap between `lane0` and `lane1` hidden by
/// `visibility` (2 `around`, 3 `between`; other values hide nothing).
/// `cuts` are the cross-gap windows of both lanes, `r0..r1` the rule extent.
pub(super) fn hidden_lane_segments(
    items: &[GapItem],
    lane0: (f32, f32),
    lane1: (f32, f32),
    cuts: &[(f32, f32)],
    r: (f32, f32),
    visibility: u8,
) -> Vec<(f32, f32)> {
    if visibility < 2 || r.1 - r.0 <= GAP_EPS {
        return vec![];
    }
    let mut stops: Vec<f32> = cuts
        .iter()
        .map(|&(lo, hi)| (lo + hi) / 2.0)
        .filter(|&c| c > r.0 + GAP_EPS && c < r.1 - GAP_EPS)
        .collect();
    stops.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    stops.dedup_by(|x, y| (*x - *y).abs() <= GAP_EPS);
    let in_lane = |i: &GapItem, (s, e): (f32, f32)| i.a0 < e - GAP_EPS && i.a1 > s + GAP_EPS;
    let side = |lane: (f32, f32), lo: f32, hi: f32| {
        items.iter().any(|i| in_lane(i, lane) && i.covers_b(lo, hi))
    };
    let mut out = vec![];
    let mut lo = r.0;
    for hi in stops.into_iter().chain(std::iter::once(r.1)) {
        let (before, after) = (side(lane0, lo, hi), side(lane1, lo, hi));
        let shown = if visibility == 2 {
            before || after
        } else {
            before && after
        };
        if !shown {
            out.push((lo, hi));
        }
        lo = hi;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(a0: f32, a1: f32, b0: f32, b1: f32) -> GapItem {
        GapItem { a0, a1, b0, b1 }
    }

    #[test]
    fn lane_ending_early_hides_tail_segments() {
        // Lane 0 holds [0,40] and [60,160]; lane 1 holds [0,70] and [90,150].
        let items = [
            item(0.0, 60.0, 0.0, 40.0),
            item(0.0, 60.0, 60.0, 160.0),
            item(80.0, 140.0, 0.0, 70.0),
            item(80.0, 140.0, 90.0, 150.0),
        ];
        let cuts = [(40.0, 60.0), (70.0, 90.0), (160.0, 180.0)];
        let hidden =
            hidden_lane_segments(&items, (0.0, 60.0), (80.0, 140.0), &cuts, (0.0, 300.0), 3);
        assert_eq!(hidden, vec![(170.0, 300.0)]);
        let around =
            hidden_lane_segments(&items, (0.0, 60.0), (80.0, 140.0), &cuts, (0.0, 300.0), 2);
        assert_eq!(around, vec![(170.0, 300.0)]);
    }

    #[test]
    fn empty_lane_hides_whole_gap_and_all_hides_nothing() {
        let items = [item(0.0, 60.0, 0.0, 60.0)];
        let hidden = hidden_lane_segments(&items, (0.0, 60.0), (80.0, 140.0), &[], (0.0, 60.0), 3);
        assert_eq!(hidden, vec![(0.0, 60.0)]);
        assert!(
            hidden_lane_segments(&items, (0.0, 60.0), (80.0, 140.0), &[], (0.0, 60.0), 1)
                .is_empty()
        );
        assert!(
            hidden_lane_segments(&items, (0.0, 60.0), (80.0, 140.0), &[], (0.0, 60.0), 2)
                .is_empty()
        );
    }
}
