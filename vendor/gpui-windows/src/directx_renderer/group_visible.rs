//! Bound the source surface by the visible frame mapped into source space.

use super::*;

pub(super) fn source(group: &gpui::PaintGroup, width: u32, height: u32) -> Bounds<ScaledPixels> {
    let Some(m) = group.projection else {
        return group.bounds;
    };
    let mut lo = [f32::MAX; 2];
    let mut hi = [f32::MIN; 2];
    // A positive denominator at every corner stays positive across the frame.
    // Crossing the horizon prevents finite bounds, so retain the source ink.
    for (x, y) in [
        (0., 0.),
        (width as f32, 0.),
        (width as f32, height as f32),
        (0., height as f32),
    ] {
        let w = m[6] * x + m[7] * y + m[8];
        if !w.is_finite() || w <= 1e-6 {
            return group.bounds;
        }
        let p = [
            (m[0] * x + m[1] * y + m[2]) / w,
            (m[3] * x + m[4] * y + m[5]) / w,
        ];
        if !p.into_iter().all(f32::is_finite) {
            return group.bounds;
        }
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let visible = Bounds::new(
        point(ScaledPixels(lo[0]), ScaledPixels(lo[1])),
        size(ScaledPixels(hi[0] - lo[0]), ScaledPixels(hi[1] - lo[1])),
    );
    group.bounds.intersect(&visible)
}

/// Bound work by the polygon the shader actually clips, not semantic geometry.
/// Unsupported or inverse polygons must retain the original write rectangle.
pub(super) fn output(q: &BlurQuad, fallback: Bounds<ScaledPixels>) -> Bounds<ScaledPixels> {
    let count = q.poly_count & 15;
    if q.poly_count & 16 != 0 || !(3..=8).contains(&count) {
        return fallback;
    }
    let mut lo = [f32::MAX; 2];
    let mut hi = [f32::MIN; 2];
    for i in 0..count as usize {
        let p = [q.poly[i / 2][(i % 2) * 2], q.poly[i / 2][(i % 2) * 2 + 1]];
        if !p.into_iter().all(f32::is_finite) {
            return fallback;
        }
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let visible = Bounds::new(
        point(ScaledPixels(lo[0] - 2.), ScaledPixels(lo[1] - 2.)),
        size(
            ScaledPixels(hi[0] - lo[0] + 4.),
            ScaledPixels(hi[1] - lo[1] + 4.),
        ),
    );
    fallback.intersect(&visible)
}

#[cfg(test)]
mod tests {
    use super::{BlurQuad, Bounds, Region, ScaledPixels, output, point, size, source};

    fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
        Bounds::new(
            point(ScaledPixels(x), ScaledPixels(y)),
            size(ScaledPixels(w), ScaledPixels(h)),
        )
    }

    #[test]
    fn shader_polygon_limits_only_work_with_a_conservative_margin() {
        let mut q = BlurQuad::default();
        q.poly_count = 3;
        q.poly[0] = [20., 30., 40., 30.];
        q.poly[1] = [20., 60., 0., 0.];
        let frame = bounds(0., 0., 800., 600.);
        assert_eq!(output(&q, frame), bounds(18., 28., 24., 34.));
        for count in [0, 2, 9, 19] {
            q.poly_count = count;
            assert_eq!(output(&q, frame), frame);
        }
        q.poly_count = 3;
        q.poly[0][0] = f32::NAN;
        assert_eq!(output(&q, frame), frame);
    }

    #[test]
    fn frame_maps_to_source_before_intersecting_ink() {
        let g = gpui::PaintGroup {
            bounds: bounds(0., 0., 2000., 2000.),
            projection: Some([2., 0., 100., 0., 2., 40., 0., 0., 1.]),
            ..Default::default()
        };
        assert_eq!(source(&g, 800, 600), bounds(100., 40., 1600., 1200.));
        let [raw, _] = Region::for_group(&g, 800, 600);
        assert_eq!(
            raw,
            Region {
                x: 98,
                y: 38,
                width: 702,
                height: 562
            }
        );
    }

    #[test]
    fn horizon_or_invalid_mapping_keeps_source_ink() {
        let mut g = gpui::PaintGroup {
            bounds: bounds(20., 30., 40., 50.),
            projection: Some([1., 0., 0., 0., 1., 0., -0.01, 0., 1.]),
            ..Default::default()
        };
        assert_eq!(source(&g, 800, 600), g.bounds);
        g.projection = Some([f32::NAN; 9]);
        assert_eq!(source(&g, 800, 600), g.bounds);
    }
}
