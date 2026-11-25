//! KaminIDE patch: the instances of a batch that can write inside a redraw
//! rectangle (`draw_scene_within`).
//!
//! The scissor already keeps every pixel outside the rectangle, but each
//! rectangle is a pass over every batch. A batch is drawn from its first to
//! its last instance whose extent meets the rectangle, and not at all when
//! none does; the instances skipped cannot reach the rectangle and write none
//! of its pixels, so the strip is the same. The range is never split: on WARP
//! a draw call costs more than the instances it would leave out.

use super::group_region::Region;
use gpui::{Bounds, ContentMask, ScaledPixels, TransformationMatrix};
use std::ops::Range;

/// The device pixels an instance can write: its bounds grown by `margin`
/// (antialiasing, a shadow's blur), within its mask; `None` when a
/// transformation moves it elsewhere (always drawn).
pub(super) fn extent(
    bounds: Bounds<ScaledPixels>,
    mask: &ContentMask<ScaledPixels>,
    transformation: Option<&TransformationMatrix>,
    margin: f32,
) -> Option<[f32; 4]> {
    if transformation.is_some_and(|t| *t != TransformationMatrix::unit()) {
        return None;
    }
    let (o, s) = (bounds.origin, bounds.size);
    let (mo, ms) = (mask.bounds.origin, mask.bounds.size);
    Some([
        (o.x.0 - margin).max(mo.x.0),
        (o.y.0 - margin).max(mo.y.0),
        (o.x.0 + s.width.0 + margin).min(mo.x.0 + ms.width.0),
        (o.y.0 + s.height.0 + margin).min(mo.y.0 + ms.height.0),
    ])
}

/// The range of `items` from the first to the last whose extent meets `rect`
/// (none: empty); without a rectangle, all of them.
pub(super) fn runs<T>(
    items: &[T],
    rect: Option<Region>,
    extent: impl Fn(&T) -> Option<[f32; 4]>,
) -> Vec<Range<usize>> {
    let Some(r) = rect else {
        return vec![0..items.len()];
    };
    let (x0, y0) = (r.x as f32, r.y as f32);
    let (x1, y1) = (x0 + r.width as f32, y0 + r.height as f32);
    let meets = |item: &T| match extent(item) {
        None => true,
        Some(e) => e[0] < x1 && e[2] > x0 && e[1] < y1 && e[3] > y0,
    };
    let Some(first) = items.iter().position(&meets) else {
        return Vec::new();
    };
    let last = items.iter().rposition(&meets).unwrap_or(first);
    vec![first..last + 1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, size};

    fn b(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
        Bounds {
            origin: point(ScaledPixels(x), ScaledPixels(y)),
            size: size(ScaledPixels(w), ScaledPixels(h)),
        }
    }

    #[test]
    fn a_batch_draws_from_its_first_to_its_last_instance_in_the_rectangle() {
        let mask = ContentMask { bounds: b(0.0, 0.0, 800.0, 600.0) };
        let boxes: Vec<_> = (0..10).map(|k| b(0.0, 60.0 * k as f32, 100.0, 20.0)).collect();
        let strip = Region { x: 0, y: 480, width: 800, height: 120 };
        let got = runs(&boxes, Some(strip), |q| extent(*q, &mask, None, 1.0));
        assert_eq!(got, vec![8..10]);
        let apart = Region { x: 0, y: 0, width: 800, height: 600 };
        let mut few = boxes.clone();
        few.swap(1, 9);
        let top = Region { x: 0, y: 0, width: 800, height: 30 };
        assert_eq!(runs(&few, Some(top), |q| extent(*q, &mask, None, 1.0)), vec![0..1]);
        assert_eq!(runs(&boxes, Some(apart), |q| extent(*q, &mask, None, 1.0)), vec![0..10]);
        let below = Region { x: 0, y: 700, width: 800, height: 10 };
        assert!(runs(&boxes, Some(below), |q| extent(*q, &mask, None, 1.0)).is_empty());
        assert_eq!(runs(&boxes, None, |q| extent(*q, &mask, None, 1.0)), vec![0..10]);
    }

    #[test]
    fn a_mask_and_a_transformation_decide_the_extent() {
        let clipped = ContentMask { bounds: b(0.0, 0.0, 800.0, 100.0) };
        assert_eq!(extent(b(0.0, 50.0, 10.0, 500.0), &clipped, None, 1.0).map(|e| e[3]), Some(100.0));
        let mut turned = TransformationMatrix::unit();
        turned.translation[1] = 500.0;
        assert_eq!(extent(b(0.0, 0.0, 10.0, 10.0), &clipped, Some(&turned), 1.0), None);
    }
}
