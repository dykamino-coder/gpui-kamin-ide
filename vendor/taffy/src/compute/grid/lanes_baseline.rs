//! Align physical baseline groups within each lane track.
use super::*;

pub(super) fn align(placed: &mut [Placed], n: usize, rows: bool) {
    if rows {
        for t in 0..n {
            let group: Vec<usize> = (0..placed.len())
                .filter(|&i| placed[i].grid_baseline && placed[i].start == t)
                .collect();
            if group.len() < 2 {
                continue;
            }
            let base_of = |p: &Placed| p.margin.top + p.baseline.unwrap_or(p.size.height);
            let shared = group
                .iter()
                .map(|&i| base_of(&placed[i]))
                .fold(f32::NEG_INFINITY, f32_max);
            for i in group {
                let shim = shared - base_of(&placed[i]);
                placed[i].grid_pos += shim;
            }
        }
        // KaminIDE patch: группа `last baseline` — по дорожке, где элемент
        // КОНЧАЕТСЯ (css-align-3 §9.1: «end-most shared alignment context»);
        // базовая меряется от конечного края поля (Blink
        // grid_lanes_layout_algorithm.cc:501-510 `GetBaselineSideMargin`:
        // у last — `block_end`), элемент уже прижат к концу области запасным
        // `safe self-end` (`align_item_within_area`) и сдвигается к началу.
        for t in 0..n {
            let group: Vec<usize> = (0..placed.len())
                .filter(|&i| placed[i].grid_last_baseline && placed[i].end == t + 1)
                .collect();
            if group.len() < 2 {
                continue;
            }
            let from_end = |p: &Placed| {
                p.margin.bottom + p.size.height - p.last_baseline.unwrap_or(p.size.height)
            };
            let shared = group
                .iter()
                .map(|&i| from_end(&placed[i]))
                .fold(f32::NEG_INFINITY, f32_max);
            for i in group {
                let shim = shared - from_end(&placed[i]);
                placed[i].grid_pos -= shim;
            }
        }
    }

    if rows {
        return;
    }
    for last in [false, true] {
        for end_side in [false, true] {
            for track in 0..n {
                let group: Vec<usize> = (0..placed.len())
                    .filter(|&i| {
                        let p = &placed[i];
                        let participates = if last {
                            p.grid_last_baseline
                        } else {
                            p.grid_baseline
                        };
                        let edge = if last { p.end - 1 } else { p.start };
                        participates
                            // Match ordinary grid's first-baseline fallback for
                            // parallel horizontal items; last groups still share the end edge.
                            && (last || p.baseline_x_flags & 8 == 0)
                            && edge == track
                            && ((p.baseline_x_flags & 1 != 0) != last) == end_side
                    })
                    .collect();
                if group.len() < 2 {
                    continue;
                }
                let distance = |p: &Placed| {
                    let own = if last {
                        p.last_baseline_x
                    } else {
                        p.baseline_x
                    };
                    let synthesized = if p.baseline_x_flags & 2 != 0 {
                        p.size.width / 2.0
                    } else {
                        0.0
                    };
                    let own = if p.baseline_x_flags & 4 != 0 {
                        own
                    } else {
                        None
                    };
                    let baseline = super::super::track_sizing::baseline_coordinate(
                        own.or(Some(synthesized)),
                        p.size.width,
                        p.overflow.x.is_scroll_container(),
                    );
                    if end_side {
                        p.margin.right + p.size.width - baseline
                    } else {
                        p.margin.left + baseline
                    }
                };
                let shared = group
                    .iter()
                    .map(|&i| distance(&placed[i]))
                    .fold(f32::NEG_INFINITY, f32_max);
                for i in group {
                    let shim = shared - distance(&placed[i]);
                    placed[i].grid_pos += if end_side { -shim } else { shim };
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tree/lanes_parallel_margin_tests.rs"]
mod tests;
