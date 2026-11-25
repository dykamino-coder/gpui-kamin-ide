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

/// The frame pixels a group composite can cover (x0, y0, x1, y1, whole
/// pixels): its image's rectangle, or for a plane the image of that
/// rectangle within the raster under the plane's homography, of which
/// `q.proj` is the inverse (frame to source, `group_source_with`). Outside
/// it the shader samples nothing, so a pixel there takes no colour. `None`
/// without an image, for a singular mapping, or when a corner lies behind
/// the eye (the image is then unbounded).
pub(super) fn footprint(q: &BlurQuad, vw: f32, vh: f32) -> Option<[f32; 4]> {
    if q.proj[2][3] <= 0.5 {
        // Without a plane the image lies at its own frame pixels.
        let s = q.source_rect;
        if !(s[2] > 0.0 && s.iter().all(|v| v.is_finite())) {
            return None;
        }
        return Some([s[0].max(0.0), s[1].max(0.0), (s[0] + s[2]).min(vw), (s[1] + s[3]).min(vh)]);
    }
    let s = q.source_rect;
    let (x0, y0) = (s[0].max(0.0), s[1].max(0.0));
    let (x1, y1) = ((s[0] + s[2]).min(vw), (s[1] + s[3]).min(vh));
    if x0 >= x1 || y0 >= y1 {
        return Some([0.0; 4]);
    }
    let m: [f32; 9] = std::array::from_fn(|k| q.proj[k / 3][k % 3]);
    let [lo_x, lo_y, hi_x, hi_y] = gpui::homography_image(&m, [x0, y0, x1, y1])?;
    // 2 px for the shader's own rounding of the map at every pixel.
    Some([
        (lo_x - 2.0).floor().max(0.0),
        (lo_y - 2.0).floor().max(0.0),
        (hi_x + 2.0).ceil().min(vw),
        (hi_y + 2.0).ceil().min(vh),
    ])
}

#[cfg(test)]
mod tests {
    use super::{BlurQuad, Bounds, Region, ScaledPixels, output, footprint, point, size, source};

    #[test]
    fn a_plane_covers_the_image_of_its_source_not_the_frame() {
        let mut q = BlurQuad::default();
        // Frame to source: source = frame / 2 - (50, 20), so the source
        // rectangle 0..100 x 0..50 lies at frame 100..300 x 40..140.
        q.proj = [[0.5, 0.0, -50.0, 0.0], [0.0, 0.5, -20.0, 0.0], [0.0, 0.0, 1.0, 1.0]];
        q.source_rect = [0.0, 0.0, 100.0, 50.0];
        assert_eq!(footprint(&q, 800.0, 600.0), Some([98.0, 38.0, 302.0, 142.0]));
        // Behind the eye at a corner: unbounded, nothing is narrowed.
        q.proj = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.02, 0.0, 1.0, 1.0]];
        assert_eq!(footprint(&q, 800.0, 600.0), None);
        // Without a plane: the image's own rectangle, within the raster.
        q.proj[2][3] = 0.0;
        q.source_rect = [700.0, 10.0, 200.0, 50.0];
        assert_eq!(footprint(&q, 800.0, 600.0), Some([700.0, 10.0, 800.0, 60.0]));
    }

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
