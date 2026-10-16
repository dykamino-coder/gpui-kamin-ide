//! Zero-sized gutters distinguish touching items from actual spanners.
use super::GapItem;
use gpui::{Bounds, point, px, size};

#[test]
fn touching_a_zero_gap_does_not_span_it() {
    let item = |start: f32, end: f32| {
        GapItem::from_bounds(
            &Bounds {
                origin: point(px(start), px(0.)),
                size: size(px(end - start), px(1.)),
            },
            true,
        )
    };
    let left = item(0., 100.);
    let right = item(100., 200.);
    let spanner = item(0., 200.);
    assert!(!left.spans_a(100., 100.));
    assert!(!right.spans_a(100., 100.));
    assert!(spanner.spans_a(100., 100.));
    assert!(left.spans_a(80., 100.));
    assert!(spanner.spans_a(100., 120.));
    assert!(!left.spans_a(100., 120.));
}
