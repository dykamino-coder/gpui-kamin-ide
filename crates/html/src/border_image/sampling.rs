//! Cache and sample border-image slices without resampling an unscaled raster.

use gpui::{Bounds, px};

/// Кэш вырезанных кусков: (образ, прямоугольник растра) → свой образ.
///
/// Кусок обязан быть ОТДЕЛЬНЫМ образом, а не регионом атласа: регион в один
/// знак, растянутый на всю сторону рамки, размывается фильтрацией с соседями
/// по атласу — зелёная кромка тонула в красной середине (`border-image-002`).
/// Вырезка на каждый кадр непозволительна, поэтому куски запоминаются.
type SliceKey = (usize, u32, u32, u32, u32, u32, u32);
type SliceCache =
    std::sync::Mutex<std::collections::HashMap<SliceKey, std::sync::Arc<gpui::RenderImage>>>;
static SLICES: std::sync::OnceLock<SliceCache> = std::sync::OnceLock::new();
const SLICE_CAP: usize = 256;

/// Нарисовать ОДИН кусок образа в своё место.
pub(super) fn paint_slice(
    window: &mut gpui::Window,
    raster: &std::sync::Arc<gpui::RenderImage>,
    image: (f32, f32),
    raster_source: bool,
    src: (f32, f32, f32, f32),
    dest: (f32, f32, f32, f32),
) {
    let (sx, sy, sw, sh) = src;
    let (dx, dy, dw, dh) = dest;
    // Источник задан в точках CSS-картинки, а вырезка — в точках РАСТРА:
    // у рисунка, растрированного плотнее, они различаются.
    let s = raster.size(0);
    let (kx, ky) = (
        s.width.0 as f32 / image.0.max(1.0),
        s.height.0 as f32 / image.1.max(1.0),
    );
    // В ключе и целевой размер: тот же кусок в другой рамке растягивается
    // иначе. Размер целевого растра — в физических точках устройства, чтобы
    // на дробном масштабе кусок не мылился повторным растяжением.
    // CSS Backgrounds 3 §6.6 scales slices only when their CSS dimensions
    // change. Keep an unscaled raster on its native pixel grid, as the
    // background painter does; pre-resampling at fractional device scale
    // otherwise blends adjacent source pixels before painting the slice.
    let unscaled = raster_source
        && sw == dw
        && sh == dh
        && [sx, sy, sw, sh].iter().all(|v| v.fract() == 0.0)
        && window.current_transformation().rotation_scale == [[1.0, 0.0], [0.0, 1.0]];
    let scale = if unscaled { 1.0 } else { window.scale_factor() };
    let (out_w, out_h) = (
        ((dw * scale).round() as u32).max(1),
        ((dh * scale).round() as u32).max(1),
    );
    let key: SliceKey = (
        std::sync::Arc::as_ptr(raster) as usize,
        (sx * kx).round() as u32,
        (sy * ky).round() as u32,
        (sw * kx).round().max(1.0) as u32,
        (sh * ky).round().max(1.0) as u32,
        out_w,
        out_h,
    );
    let cache = SLICES.get_or_init(Default::default);
    let piece = {
        let hit = cache.lock().ok().and_then(|m| m.get(&key).cloned());
        match hit {
            Some(found) => found,
            None => {
                let Some(cut) = gpui::crop_image(raster, key.1, key.2, key.3, key.4, out_w, out_h)
                else {
                    return;
                };
                if let Ok(mut m) = cache.lock() {
                    if m.len() >= SLICE_CAP {
                        m.clear();
                    }
                    m.insert(key, cut.clone());
                }
                cut
            }
        }
    };
    let cell = Bounds {
        origin: gpui::point(px(dx), px(dy)),
        size: gpui::size(px(dw), px(dh)),
    };
    let sampling = if unscaled {
        gpui::ImageSampling::Nearest
    } else {
        gpui::ImageSampling::Linear
    };
    let _ =
        window.paint_image_with_sampling(cell, gpui::Corners::default(), piece, 0, false, sampling);
}
