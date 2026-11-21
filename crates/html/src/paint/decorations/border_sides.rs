//! Border sides of different colours on square corners, each painted as its
//! own trapezoid (css-backgrounds-3 §4.4 "Corner Clipping" / §5 transitions:
//! adjoining sides meet on the line from the outer corner of the border box
//! to the inner corner of the padding box). With zero width or height and
//! transparent sides the trapezoids become the triangles authors draw with
//! borders. Blink clips each side to the same quadrilateral
//! (`BoxBorderPainter::ClipBorderSidePolygon`), Gecko builds the same corner
//! split (`nsCSSBorderRenderer::GetSideClipWithoutCornersRect` and the
//! corner triangles); here the polygon is filled directly.

use gpui::{AnyElement, Bounds, Hsla, IntoElement, PathBuilder, Pixels, Styled, point, px};

type Quad = [(f32, f32); 4];

/// Extend the earlier opaque side underneath the later one at their shared
/// diagonal. Separate antialiased fills otherwise leave background coverage
/// between them (even though the mathematical polygons tile exactly).
/// Like Gecko's corner skirts, the overlap stays inside the corner rectangle.
/// Transparent joins keep their partition: overpainting would compound alpha.
fn outline(
    quads: &[Quad; 4],
    i: usize,
    colours: &[Option<Hsla>; 4],
    pixel: f32,
) -> Vec<(f32, f32)> {
    let q = quads[i];
    let mut out = Vec::with_capacity(8);
    let opaque = |j: usize| colours[j].is_some_and(|c| c.a == 1.0);
    let skirt = |outer: (f32, f32), inner: (f32, f32)| {
        let dx = (outer.0 - inner.0).abs();
        let dy = (outer.1 - inner.1).abs();
        if dx == 0.0 || dy == 0.0 || q[2] == q[3] {
            return (outer, inner);
        }
        // Displace the shared diagonal by one device pixel, intersecting its
        // parallel with the outer and inner edges of this corner rectangle.
        let distance = dx.hypot(dy) * pixel;
        let sx = (distance / dy).min(dx);
        let sy = (distance / dx).min(dy);
        match i {
            0 | 2 => (
                (outer.0, outer.1 + (inner.1 - outer.1).signum() * sy),
                (inner.0 + (outer.0 - inner.0).signum() * sx, inner.1),
            ),
            _ => (
                (outer.0 + (inner.0 - outer.0).signum() * sx, outer.1),
                (inner.0, inner.1 + (outer.1 - inner.1).signum() * sy),
            ),
        }
    };
    out.push(q[0]);
    out.push(q[1]);
    let next = (i + 1) % 4;
    if next > i && opaque(i) && opaque(next) {
        let (a, b) = skirt(q[1], q[2]);
        out.extend([a, b]);
    }
    out.extend([q[2], q[3]]);
    let prev = (i + 3) % 4;
    if prev > i && opaque(i) && opaque(prev) {
        let (a, b) = skirt(q[0], q[3]);
        out.extend([b, a]);
    }
    out
}

/// The four side polygons (top, right, bottom, left) of a border box of
/// `w`×`h` with side widths `[t, r, b, l]`, in the box's own coordinates:
/// outer edge first, then the inner edge back.
pub(crate) fn side_polygons(w: f32, h: f32, [t, r, b, l]: [f32; 4]) -> [Quad; 4] {
    // Inner corners of the padding box; when opposite sides overlap
    // (the padding box is empty), the meeting point is shared.
    let ix0 = l.min(w);
    let iy0 = t.min(h);
    let ix1 = (w - r).max(ix0);
    let iy1 = (h - b).max(iy0);
    // An overlapping pair splits the overlap in proportion to its widths.
    let (ix0, ix1) = if l + r > w && l + r > 0.0 {
        let x = w * l / (l + r);
        (x, x)
    } else {
        (ix0, ix1)
    };
    let (iy0, iy1) = if t + b > h && t + b > 0.0 {
        let y = h * t / (t + b);
        (y, y)
    } else {
        (iy0, iy1)
    };
    [
        [(0.0, 0.0), (w, 0.0), (ix1, iy0), (ix0, iy0)],
        [(w, 0.0), (w, h), (ix1, iy1), (ix1, iy0)],
        [(w, h), (0.0, h), (ix0, iy1), (ix1, iy1)],
        [(0.0, h), (0.0, 0.0), (ix0, iy0), (ix0, iy1)],
    ]
}

/// One layer over the border box painting the sides `paint` (solid sides of
/// different colours) as trapezoids. `widths` are `[t, r, b, l]` in CSS px;
/// the layer is an absolute child of the padding box pushed out by them.
pub(crate) fn polygon_sides(widths: [f32; 4], colours: [Option<Hsla>; 4]) -> AnyElement {
    let [t, r, b, l] = widths;
    gpui::canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, _, window, _| {
            let matrix = window.current_transformation();
            let [[a, c], [bb, d]] = matrix.rotation_scale;
            let [x, y] = matrix.translation;
            let scale = window.scale_factor();
            let transform = gpui::Transform::new(a, bb, c, d, x / scale, y / scale);
            let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let (ox, oy) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
            let quads = side_polygons(w, h, widths);
            let transform_scale = a.hypot(bb).min(c.hypot(d));
            let pixel = 1.0 / (scale * transform_scale).max(f32::EPSILON);
            for (i, colour) in colours.iter().copied().enumerate() {
                let Some(colour) = colour.filter(|c| c.a > 0.0) else {
                    continue;
                };
                let quad = outline(&quads, i, &colours, pixel);
                let mut path = PathBuilder::fill();
                path.move_to(point(px(ox + quad[0].0), px(oy + quad[0].1)));
                for &(qx, qy) in &quad[1..] {
                    path.line_to(point(px(ox + qx), px(oy + qy)));
                }
                path.close();
                path.transform(transform);
                if let Ok(path) = path.build() {
                    window.paint_path(path, colour);
                }
            }
        },
    )
    .absolute()
    .top(px(-t))
    .left(px(-l))
    .right(px(-r))
    .bottom(px(-b))
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(q: &Quad) -> f32 {
        let mut s = 0.0;
        for i in 0..4 {
            let (a, b) = (q[i], q[(i + 1) % 4]);
            s += a.0 * b.1 - b.0 * a.1;
        }
        s.abs() / 2.0
    }

    #[test]
    fn zero_size_box_splits_into_four_triangles_meeting_at_the_centre() {
        let quads = side_polygons(200.0, 200.0, [100.0; 4]);
        for q in &quads {
            assert_eq!(area(q), 10000.0);
            assert!(q.contains(&(100.0, 100.0)));
        }
    }

    #[test]
    fn bottom_only_triangle_has_its_apex_on_the_top_edge() {
        // `width:0; height:0; border-width: 0 100px 150px 100px`.
        let [_, _, bottom, _] = side_polygons(200.0, 150.0, [0.0, 100.0, 150.0, 100.0]);
        assert_eq!(
            bottom,
            [(200.0, 150.0), (0.0, 150.0), (100.0, 0.0), (100.0, 0.0)]
        );
        assert_eq!(area(&bottom), 15000.0);
    }

    #[test]
    fn sides_tile_the_ring_of_a_box_with_content() {
        let quads = side_polygons(30.0, 20.0, [2.0, 3.0, 4.0, 5.0]);
        let total: f32 = quads.iter().map(area).sum();
        assert!((total - (30.0 * 20.0 - 22.0 * 14.0)).abs() < 1e-3);
    }
}
