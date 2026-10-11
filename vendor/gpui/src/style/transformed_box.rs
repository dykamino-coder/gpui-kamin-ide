//! Resolve axis-preserving CSS transforms before snapping a box fill to device edges.

use crate::{Bounds, Pixels, Style, Window, point, px};

pub(crate) fn bounds(style: &Style, bounds: Bounds<Pixels>, window: &Window) -> Bounds<Pixels> {
    if !style.css_border_snap
        || style.border_widths.any(|l| !l.is_zero())
        || style.corner_radii.to_pixels(window.rem_size()).max() != Pixels::ZERO
    {
        return bounds;
    }
    let paint = window.current_transformation();
    let exact_transform = window
        .css_fill_transform
        .filter(|(current, _)| *current == paint);
    let m = exact_transform.map_or(paint, |(_, exact)| exact);
    let [[a, b], [c, d]] = m.rotation_scale;
    let diagonal = b.abs() <= 1e-5
        && c.abs() <= 1e-5
        && a.abs() > 1e-5
        && d.abs() > 1e-5
        && ((a - 1.0).abs() >= 1e-5 || (d - 1.0).abs() >= 1e-5 || m.translation == [0.0, 0.0]);
    // CSS solid fills snap their final device edges, including boxes moved
    // after layout (anchor positioning). Snapping before that relocation
    // leaves fractional device edges even when the transform is identity.
    // Diagonal reflections, including flattened 3D planes, preserve axes.
    // Signed axis permutations also preserve rectangles. They must snap both
    // transformed edges, rather than move an already-rounded local box.
    if !diagonal && exact_transform.is_none() {
        return bounds;
    }
    let det = a * d - b * c;
    if !det.is_finite() || det.abs() <= 1e-5 {
        return bounds;
    }
    let Some((_, exact)) = window
        .css_exact_bounds
        .filter(|(snapped, _)| *snapped == bounds)
    else {
        return bounds;
    };
    // CSS Transforms 1 section 3 maps the CSS reference box before rasterization.
    // Undo the actual renderer map after snapping the desired device edges.
    let sf = window.scale_factor();
    let [[pa, pb], [pc, pd]] = paint.rotation_scale;
    let pdet = pa * pd - pb * pc;
    let edge = |x: Pixels, y: Pixels| {
        let (x, y) = (f32::from(x) * sf, f32::from(y) * sf);
        // Round half up on a 1/64 grid first: f32 noise of a long transform
        // chain (offset-path ray) put an exact half pixel at 197.4999 and
        // rounded it down while the equivalent plain `rotate()` rounded up
        // (`offset-path-ray-007/020/021`); the quarter-turn path in
        // `crates/html` snaps the same way.
        let snap = |v: f32| ((v * 64.0).round() / 64.0 + 0.5).floor();
        let dx = snap(a * x + b * y + m.translation[0]) - paint.translation[0];
        let dy = snap(c * x + d * y + m.translation[1]) - paint.translation[1];
        point(
            px((pd * dx - pb * dy) / (pdet * sf)),
            px((-pc * dx + pa * dy) / (pdet * sf)),
        )
    };
    Bounds::from_corners(
        edge(exact.left(), exact.top()),
        edge(exact.right(), exact.bottom()),
    )
}
