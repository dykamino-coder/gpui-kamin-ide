//! Paint affine-compatible gradient slices before any intermediate quantization.
//! CSS Backgrounds 3 §6.6 scales the source image, rather than rebuilding a
//! gradient in each slice. Clip a virtual whole image in the slice's mapping.

use crate::document::state::ValueCx;
use crate::paint::background::Source;
use gpui::{Bounds, Corners, Window, point, px, size};

pub(super) fn paint(
    window: &mut Window,
    source: &Source,
    image: (f32, f32),
    (sx, sy, sw, sh): (f32, f32, f32, f32),
    (dx, dy, dw, dh): (f32, f32, f32, f32),
    vcx: ValueCx<'_>,
) -> bool {
    let Source::Gradient { raw } = source else {
        return false;
    };
    let Some(g) = crate::style::computed::parse_gradient(raw, vcx) else {
        return false;
    };
    if g.radial || raw.starts_with("repeating-") {
        return false;
    }
    let (kx, ky) = (dw / sw, dh / sh);
    let angle = crate::style::computed::corner_angle(raw, image).unwrap_or(g.angle_deg);
    let axis_aligned = (angle / 90.0 - (angle / 90.0).round()).abs() < 0.0001;
    if (!axis_aligned && (kx - ky).abs() > 0.0001)
        || ((!g.stops_px.is_empty() || !g.stops_raw.is_empty()) && (kx != 1.0 || ky != 1.0))
    {
        return false;
    }
    let bounds = Bounds::new(
        point(px(dx - sx * kx), px(dy - sy * ky)),
        size(px(image.0 * kx), px(image.1 * ky)),
    );
    let clip = super::sampling::destination_bounds(window, (dx, dy, dw, dh));
    window.with_content_mask(Some(gpui::ContentMask { bounds: clip }), |window| {
        crate::paint::background::paint_gradient(window, bounds, Corners::default(), source, vcx)
    })
}
