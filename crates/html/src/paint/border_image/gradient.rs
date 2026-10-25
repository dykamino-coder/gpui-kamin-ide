//! Кусок `border-image` из линейного градиента красится аналитически, без промежуточного
//! квантования в растр: CSS Backgrounds 3 §6.6 масштабирует исходную картинку, а не строит
//! градиент заново в каждом куске, поэтому виртуальная целая картинка рисуется в отображении
//! куска и обрезается его назначением. Радиальные, повторяющиеся и несовместимые с аффинным
//! отображением случаи уходят на растровый путь.

use crate::document::state::ValueCx;
use crate::paint::background::Source;
use crate::resources::css_images::Source as CpuSource;
use gpui::{Bounds, Corners, Window, point, px, size};

pub(super) fn paint(
    window: &mut Window,
    source: &Source,
    image: (f32, f32),
    (sx, sy, sw, sh): (f32, f32, f32, f32),
    (dx, dy, dw, dh): (f32, f32, f32, f32),
    vcx: ValueCx<'_>,
) -> bool {
    let Source::Image(CpuSource::Gradient { image: value }) = source else {
        return false;
    };
    let Some((g, repeating)) = value.gradient() else {
        return false;
    };
    if g.radial || repeating {
        return false;
    }
    let (kx, ky) = (dw / sw, dh / sh);
    let angle = g.shape.corner_angle(image).unwrap_or(g.angle_deg);
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
