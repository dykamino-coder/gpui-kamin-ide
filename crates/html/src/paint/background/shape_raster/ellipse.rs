//! Растеризация эллипса в пиксельной сетке.

use gpui::RenderImage;
use std::sync::Arc;

/// Альфа-растр эллипса по готовым параметрам в точках растра.
pub fn rasterize_ellipse_px(
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    w: u32,
    h: u32,
) -> Option<Arc<RenderImage>> {
    if rx <= 0.0 || ry <= 0.0 {
        // Нулевой радиус — всё скрыто: прозрачная маска.
        return gpui::bgra_bytes_to_image(w, h, vec![0u8; (w * h * 4) as usize]);
    }
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 + 0.5 - cx) / rx;
            let dy = (y as f32 + 0.5 - cy) / ry;
            let d = (dx * dx + dy * dy).sqrt();
            // Расстояние до края в ТОЧКАХ: неявная функция d-1, её градиент
            // по точкам даёт локальный масштаб — без него сглаживание на
            // вытянутом эллипсе было бы шире с одной стороны.
            let grad = ((dx / rx) * (dx / rx) + (dy / ry) * (dy / ry)).sqrt() / d.max(1e-6);
            let px_dist = (d - 1.0) / grad.max(1e-6);
            let a = (0.5 - px_dist).clamp(0.0, 1.0);
            let v = (a * 255.0).round() as u8;
            bytes.extend_from_slice(&[v, v, v, v]);
        }
    }
    gpui::bgra_bytes_to_image(w, h, bytes)
}
