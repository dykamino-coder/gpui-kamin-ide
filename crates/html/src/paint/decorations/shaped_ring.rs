//! Кольцо рамки при фигурных углах (css-borders-4 §corner-shape-rendering):
//! область между внешним контуром (им режется вся группа элемента) и
//! внутренним. Прямолинейный внутренний контур до восьми вершин — обратная
//! обрезка группы многоугольником (шейдер `clip-path: polygon()`), прочее —
//! цветной растр; углы только с выемками по осям — прежний `linear_corners`.

use crate::style::values::value::Color;
use gpui::{Bounds, Pixels, Window, px};

/// Рисует кольцо по записи `rrect(... / t r b l)` в border-box `bounds`.
pub(super) fn paint(spec: &str, colour: Color, bounds: Bounds<Pixels>, window: &mut Window) {
    use crate::paint::background::Ring;
    let sf = window.scale_factor();
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let (pw, ph) = (
        (w * sf).round().max(1.0) as u32,
        (h * sf).round().max(1.0) as u32,
    );
    let args = spec.trim_start_matches("rrect(").trim_end_matches(')');
    let image = match crate::paint::background::contour_ring(args, pw, ph, sf, colour) {
        Some(Ring::Outside(pts)) => {
            let o = bounds.origin;
            let poly: Vec<_> = pts
                .iter()
                .map(|&(x, y)| gpui::point(o.x + px(x), o.y + px(y)))
                .collect();
            let fill = gpui::fill(bounds, colour.to_hsla());
            window.paint_group_outside_polygon(bounds, &poly, |w| w.paint_quad(fill));
            return;
        }
        Some(Ring::Image(img)) => Some(img),
        None => crate::paint::background::rasterize_ring_at(
            args,
            pw,
            ph,
            sf,
            colour,
            (f32::from(bounds.origin.x), f32::from(bounds.origin.y)),
        ),
    };
    if let Some(img) = image {
        let _ = window.paint_image_with_sampling(
            bounds,
            gpui::Corners::default(),
            img,
            0,
            false,
            gpui::ImageSampling::Linear,
        );
    }
}
