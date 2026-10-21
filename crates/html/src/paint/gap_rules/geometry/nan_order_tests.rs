//! Gap-rule geometry sorts coordinates with a total order: a NaN coordinate
//! must neither panic the sort (Rust detects comparators that are not a total
//! order on slices of 21+ elements) nor leave the finite values unsorted.

use super::{GapItem, grid, lines, tracks, uniq_sorted};

/// 64 coordinates with every third one NaN, in scrambled order.
fn scrambled() -> Vec<f32> {
    (0..64)
        .map(|i| {
            if i % 3 == 0 {
                f32::NAN
            } else {
                ((i * 37) % 101) as f32
            }
        })
        .collect()
}

fn finite_sorted(v: &[f32]) -> bool {
    let finite: Vec<f32> = v.iter().copied().filter(|x| !x.is_nan()).collect();
    finite.windows(2).all(|w| w[0] <= w[1])
}

#[test]
fn uniq_sorted_orders_finite_values_around_nan() {
    let out = uniq_sorted(scrambled());
    assert!(finite_sorted(&out), "{out:?}");
}

#[test]
fn merged_spans_and_tracks_tolerate_nan_starts() {
    let spans: Vec<(f32, f32)> = scrambled().into_iter().map(|s| (s, s + 0.5)).collect();
    let merged = grid::merge(spans.clone());
    let starts: Vec<f32> = merged.iter().map(|s| s.0).collect();
    assert!(finite_sorted(&starts), "{starts:?}");
    let kept = tracks::uncollapsed(spans, 4.0);
    let starts: Vec<f32> = kept.iter().map(|s| s.0).collect();
    assert!(finite_sorted(&starts), "{starts:?}");
}

#[test]
fn flex_line_groups_tolerate_nan_extents() {
    let items: Vec<GapItem> = scrambled()
        .into_iter()
        .map(|a0| GapItem {
            a0,
            a1: a0 + 0.5,
            b0: 0.0,
            b1: 1.0,
            margin_end: 0.0,
        })
        .collect();
    let groups = lines::line_groups(&items);
    let starts: Vec<f32> = groups.iter().map(|g| g.0).collect();
    assert!(finite_sorted(&starts), "{starts:?}");
}
