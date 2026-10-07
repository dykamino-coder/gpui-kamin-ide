//! Compose mask images at source resolution before sampling the final mask.

use gpui::RenderImage;
use std::sync::Arc;

/// Слой готового полотна маски: растр плитки и её укладка в device px.
pub struct MaskLayer {
    pub image: Arc<RenderImage>,
    /// Угол и размер плитки в точках полотна.
    pub tile: [f32; 4],
    /// Пооосный запрет мощения.
    pub no_repeat: (bool, bool),
    /// Оператор с накопленным низом: 0 add, 1 subtract, 2 intersect, 3 exclude.
    pub op: u8,
    /// Светимость вместо альфы (`mask-mode: luminance`, SVG `<mask>`).
    pub luminance: bool,
}

/// Сложить слои маски в одно полотно (css-masking §7.10.2, `mask-composite`).
///
/// Слои компонуются С НИЖНЕГО (последнего в списке): оператор каждого
/// действует между ним и стопкой под ним. Выход — альфа-полотно размером с
/// коробку; примитивам выше оно уходит одной плиткой без мощения.
pub fn compose_mask_layers(layers: &[MaskLayer], w: u32, h: u32) -> Option<Arc<RenderImage>> {
    if layers.is_empty() {
        return None;
    }
    // CSS Masking 1 §7.10.2: compose the mask images before applying the
    // resulting mask to the element. Retain the source raster resolution
    // until that final sampling step; nearest sampling into a device-sized
    // intermediate discards fractional coverage at SVG/image edges. Keep
    // at least device resolution so low-resolution images are not reduced
    // again after their existing mask-size mapping.
    let density = layers
        .iter()
        .filter_map(|layer| {
            let size = layer.image.size(0);
            let [_, _, tw, th] = layer.tile;
            (tw > 0.0 && th > 0.0)
                .then(|| (size.width.0 as f32 / tw).max(size.height.0 as f32 / th))
        })
        .fold(1.0f32, f32::max);
    let density = density
        .max(f32::EPSILON)
        .min(4096.0 / w.max(h).max(1) as f32);
    let (rw, rh) = (
        (w as f32 * density).round().max(1.0) as u32,
        (h as f32 * density).round().max(1.0) as u32,
    );
    let (sx, sy) = (rw as f32 / w.max(1) as f32, rh as f32 / h.max(1) as f32);
    let (w, h) = (rw, rh);
    let mut acc = vec![0.0f32; (w * h) as usize];
    let mut first = true;
    for layer in layers.iter().rev() {
        let bytes = layer.image.as_bytes(0)?;
        let size = layer.image.size(0);
        let (iw, ih) = (size.width.0.max(1) as usize, size.height.0.max(1) as usize);
        let [tx, ty, tw, th] = layer.tile;
        let (tx, ty, tw, th) = (tx * sx, ty * sy, tw * sx, th * sy);
        if tw <= 0.0 || th <= 0.0 {
            continue;
        }
        for y in 0..h as usize {
            for x in 0..w as usize {
                let mut u = (x as f32 + 0.5 - tx) / tw;
                let mut v = (y as f32 + 0.5 - ty) / th;
                let outside = (layer.no_repeat.0 && !(0.0..1.0).contains(&u))
                    || (layer.no_repeat.1 && !(0.0..1.0).contains(&v));
                let a = if outside {
                    0.0
                } else {
                    u = u.rem_euclid(1.0);
                    v = v.rem_euclid(1.0);
                    let px_ = ((u * iw as f32) as usize).min(iw - 1);
                    let py = ((v * ih as f32) as usize).min(ih - 1);
                    let at4 = (py * iw + px_) * 4;
                    if layer.luminance {
                        // RenderImage stores straight BGRA; CSS Masking §7.10.1
                        // multiplies the luminance by its alpha coverage.
                        (bytes[at4] as f32 * 0.0722
                            + bytes[at4 + 1] as f32 * 0.7152
                            + bytes[at4 + 2] as f32 * 0.2126)
                            * bytes[at4 + 3] as f32
                            / (255.0 * 255.0)
                    } else {
                        bytes[at4 + 3] as f32 / 255.0
                    }
                };
                let at = y * w as usize + x;
                let d = acc[at];
                acc[at] = if first {
                    a
                } else {
                    match layer.op {
                        1 => a * (1.0 - d),
                        2 => a * d,
                        3 => a * (1.0 - d) + d * (1.0 - a),
                        _ => a + d * (1.0 - a),
                    }
                };
            }
        }
        first = false;
    }
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for a in acc {
        let v = (a * 255.0) as u8;
        bytes.extend_from_slice(&[v, v, v, v]);
    }
    gpui::bgra_bytes_to_image(w, h, bytes)
}
