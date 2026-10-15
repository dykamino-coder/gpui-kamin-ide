//! Кольцо рамки при фигурных углах (css-borders-4 §corner-shape-rendering):
//! область между внешним контуром (им режется вся группа элемента) и
//! внутренним. Прямолинейный внутренний контур до восьми вершин — обратная
//! обрезка группы многоугольником (шейдер `clip-path: polygon()`), прочее —
//! цветной растр; углы только с выемками по осям — прежний `linear_corners`.

use crate::style::computed::Computed;
use crate::style::values::value::{Color, Len};
use gpui::{Bounds, Pixels, Window, px};

/// Рисует кольцо по записи `rrect(... / t r b l)` в border-box `bounds`.
pub(crate) fn paint(spec: &str, colour: Color, bounds: Bounds<Pixels>, window: &mut Window) {
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

/// Запись кольца, его цвет и толщины t/r/b/l; None — углы не фигурные,
/// рамки нет или стороны разного цвета (они остаются полосами).
pub(crate) fn spec(c: &Computed) -> Option<(String, Color, [f32; 4])> {
    if !c.corner_shaped() {
        return None;
    }
    let sides: Vec<_> = c.border_colors.iter().flatten().collect();
    let uniform = sides
        .first()
        .filter(|f| sides.iter().all(|s| s == *f))
        .map(|f| **f);
    let mixed = sides.len() > 1 && uniform.is_none();
    let side_px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let bw = c.borders();
    let widths = [
        side_px(bw.top),
        side_px(bw.right),
        side_px(bw.bottom),
        side_px(bw.left),
    ];
    if mixed || widths.iter().all(|w| *w <= 0.0) {
        return None;
    }
    // Без цвета рамка красится цветом текста, без него — чёрным
    // (начальное `border-color: currentColor`).
    let colour = uniform.or(c.border_color).or(c.color).unwrap_or(Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    });
    let colour = crate::paint::background::border_paint(c, colour);
    Some((
        crate::paint::background::rrect_spec(c, Some(widths)),
        colour,
        widths,
    ))
}
