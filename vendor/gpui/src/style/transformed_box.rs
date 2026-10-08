//! Resolve axis-preserving CSS transforms before snapping a box fill to device edges.

use crate::{Bounds, Pixels, Style, Window, point, px};

pub(super) fn bounds(style: &Style, bounds: Bounds<Pixels>, window: &Window) -> Bounds<Pixels> {
    if !style.css_border_snap
        || style.border_widths.any(|l| !l.is_zero())
        || style.corner_radii.to_pixels(window.rem_size()).max() != Pixels::ZERO
    {
        return bounds;
    }
    let m = window.current_transformation();
    let [[a, b], [c, d]] = m.rotation_scale;
    // Only diagonal scaling is handled here. Rotated quads keep their existing
    // raster path, and ordinary GPUI widgets do not opt into CSS box snapping.
    if b.abs() > 1e-5
        || c.abs() > 1e-5
        || a <= 1e-5
        || d <= 1e-5
        || (a - 1.0).abs() < 1e-5 && (d - 1.0).abs() < 1e-5
        || !a.is_finite()
        || !d.is_finite()
    {
        return bounds;
    }
    let Some((_, exact)) = window
        .css_exact_bounds
        .filter(|(snapped, _)| *snapped == bounds)
    else {
        return bounds;
    };
    // CSS Transforms 1 §3 applies the matrix to the CSS reference box. Snapping
    // 50px * 1.25 to 63 before scale(2) turns a 125-device-pixel box into 126.
    // Resolve the transformed edge first, then bring it back to local coordinates
    // for the renderer to apply this same matrix once.
    let sf = window.scale_factor();
    let edge = |v: Pixels, scale: f32, shift: f32| {
        px(((f32::from(v) * sf * scale + shift).round() - shift) / (sf * scale))
    };
    Bounds::from_corners(
        point(
            edge(exact.left(), a, m.translation[0]),
            edge(exact.top(), d, m.translation[1]),
        ),
        point(
            edge(exact.right(), a, m.translation[0]),
            edge(exact.bottom(), d, m.translation[1]),
        ),
    )
}
