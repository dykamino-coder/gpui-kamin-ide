//! css-gaps-1 §visibility-items для главных (по оси сетки) промежутков лент, блокировка
//! главного промежутка элементом в несколько лент и окно поперечного промежутка перед записью.
//!
//! Главный промежуток между лентами `k` и `k + 1` режется на отрезки в каждом поперечном
//! промежутке любой из лент (Blink `GridLanesMainGapSegmentWalker::Next`: концы отрезков —
//! центры поперечных промежутков обеих соседних лент). Сторона отрезка занята, когда элемент
//! этой ленты (элемент в несколько лент считается в каждой) перекрывает отрезок по оси укладки;
//! лента, кончившаяся раньше, оставляет отрезки за последним элементом пустыми
//! (`column-gap-decorations-rule-visibility-items-between-005`). Поперечные промежутки не
//! фильтруются: свойство действует только на линейки по оси сетки в лентах.

use super::{GAP_RULE_EPS, GapItem};

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
    if visibility < 2 || r.1 - r.0 <= GAP_RULE_EPS {
        return vec![];
    }
    let mut stops: Vec<f32> = cuts
        .iter()
        .map(|&(lo, hi)| (lo + hi) / 2.0)
        .filter(|&c| c > r.0 + GAP_RULE_EPS && c < r.1 - GAP_RULE_EPS)
        .collect();
    stops.sort_by(f32::total_cmp);
    stops.dedup_by(|x, y| (*x - *y).abs() <= GAP_RULE_EPS);
    let in_lane =
        |i: &GapItem, (s, e): (f32, f32)| i.a0 < e - GAP_RULE_EPS && i.a1 > s + GAP_RULE_EPS;
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

/// Part of a main gap blocked by spanner `i` (Blink
/// `GridLanesGapAccumulator::MarkBlockedMainGapSegments`): a lane entry runs
/// from the cross-gap centre before the item to the next one (or the rule
/// end), and a segment is blocked where the spanner is the entry on BOTH
/// sides — so the empty stretch under a dense-packed spanner stays blocked
/// up to the next cross gap (`column-gap-decorations-028/029`).
pub(super) fn lane_entry_block(
    i: &GapItem,
    lane0: &[(f32, f32)],
    lane1: &[(f32, f32)],
    r: (f32, f32),
    items: &[GapItem],
    lanes: [(f32, f32); 2],
) -> (f32, f32) {
    let entry = |cuts: &[(f32, f32)], (s, e): (f32, f32)| {
        let mut row: Vec<_> = items
            .iter()
            .filter(|v| v.a0 < e - GAP_RULE_EPS && v.a1 > s + GAP_RULE_EPS)
            .collect();
        row.sort_by(|x, y| x.b0.total_cmp(&y.b0));
        let index = row.iter().position(|v| std::ptr::eq(*v, i)).unwrap_or(0);
        let centre = |c: &(f32, f32)| (c.0 + c.1) / 2.0;
        let start = index
            .checked_sub(1)
            .and_then(|j| cuts.get(j))
            .map_or(r.0, centre);
        let end = cuts.get(index).map_or(r.1, centre);
        (start.min(i.b0), end.max(i.b1))
    };
    let (s0, e0) = entry(lane0, lanes[0]);
    let (s1, e1) = entry(lane1, lanes[1]);
    (s0.max(s1), e0.min(e1))
}

/// Cross gap of a lane in front of entry `next` (Blink `FinalGutterCenter`:
/// `ForwardStackingStart() - gap / 2`, the position BEFORE stacking-axis
/// self-alignment). The forward start is the largest running position of the
/// lanes the item covers: the end of the entry above it in each lane plus
/// the gap. Self-alignment only moves an item inside the room in front of the
/// next spanner, so the box edge can sit further down but never above it
/// (`column-gap-decorations-022/023/041`).
pub(super) fn cross_window(
    next: &GapItem,
    g: f32,
    lanes: &[(f32, f32)],
    items: &[GapItem],
) -> (f32, f32) {
    let covers =
        |i: &GapItem, (s, e): (f32, f32)| i.a0 < e - GAP_RULE_EPS && i.a1 > s + GAP_RULE_EPS;
    let mut running = f32::NEG_INFINITY;
    for &lane in lanes.iter().filter(|&&l| covers(next, l)) {
        let above = items
            .iter()
            .filter(|i| !std::ptr::eq(*i, next) && covers(i, lane) && i.b0 < next.b0 - GAP_RULE_EPS)
            .max_by(|x, y| x.b0.total_cmp(&y.b0));
        if let Some(above) = above {
            running = running.max(above.b1 + above.margin_end + g);
        }
    }
    let start = if running.is_finite() {
        running
    } else {
        next.b0
    };
    (start - g, start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(a0: f32, a1: f32, b0: f32, b1: f32) -> GapItem {
        GapItem {
            a0,
            a1,
            b0,
            b1,
            margin_end: 0.0,
        }
    }

    #[test]
    fn negative_margins_keep_gutters_attached_to_their_lane_entries() {
        let mut items = [
            item(0., 80., 0., 40.),
            item(120., 200., 0., 45.),
            item(0., 80., 80., 100.),
            item(0., 200., 100., 120.),
            item(0., 80., 120., 160.),
            item(120., 200., 120., 160.),
        ];
        items[2].margin_end = -40.;
        items[3].margin_end = -40.;
        let lanes = [(0., 80.), (120., 200.)];
        assert_eq!(cross_window(&items[3], 40., &lanes, &items), (60., 100.));
        assert_eq!(cross_window(&items[4], 40., &lanes, &items), (80., 120.));
        assert_eq!(
            lane_entry_block(
                &items[3],
                &[(40., 80.), (60., 100.), (80., 120.)],
                &[(60., 100.), (80., 120.)],
                (0., 160.),
                &items,
                lanes
            ),
            (80., 120.)
        );
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
