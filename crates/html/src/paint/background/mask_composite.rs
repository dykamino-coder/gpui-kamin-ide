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
    /// Gap after each tile per axis (`space`), canvas points.
    pub gap: (f32, f32),
    /// Tile edges snap to the device grid one copy at a time (css-masking-1
    /// §7.7-7.8 via background image geometry; Blink
    /// background_image_geometry.cc snaps each destination tile).
    pub snap: bool,
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
    let (target_w, target_h) = (w, h);
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
                if layer.snap {
                    let [dx, dy, dw, dh] = layer.tile;
                    let su = snapped_axis(
                        (x as f32 + 0.5) / sx,
                        dx,
                        dw,
                        layer.gap.0,
                        layer.no_repeat.0,
                    );
                    let sv = snapped_axis(
                        (y as f32 + 0.5) / sy,
                        dy,
                        dh,
                        layer.gap.1,
                        layer.no_repeat.1,
                    );
                    let a = match (su, sv) {
                        (Some(u), Some(v)) => {
                            let px_ = ((u * iw as f32) as usize).min(iw - 1);
                            let py = ((v * ih as f32) as usize).min(ih - 1);
                            sample(bytes, (py * iw + px_) * 4, layer.luminance)
                        }
                        _ => 0.0,
                    };
                    let at = y * w as usize + x;
                    let d = acc[at];
                    acc[at] = if first { a } else { combine(layer.op, a, d) };
                    continue;
                }
                let mut u = (x as f32 + 0.5 - tx) / tw;
                let mut v = (y as f32 + 0.5 - ty) / th;
                // `space`: tiles repeat with period tile + gap; the gap is empty.
                let (gx, gy) = (layer.gap.0 * sx, layer.gap.1 * sy);
                let mut in_gap = false;
                if gx > 0.0 && !layer.no_repeat.0 {
                    let local = (x as f32 + 0.5 - tx).rem_euclid(tw + gx);
                    in_gap |= local >= tw;
                    u = local / tw;
                }
                if gy > 0.0 && !layer.no_repeat.1 {
                    let local = (y as f32 + 0.5 - ty).rem_euclid(th + gy);
                    in_gap |= local >= th;
                    v = local / th;
                }
                let outside = in_gap
                    || (layer.no_repeat.0 && !(0.0..1.0).contains(&u))
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
    let image = gpui::bgra_bytes_to_image(w, h, bytes)?;
    // CSS Masking 1 §7.10.2: retain source resolution through all operators,
    // then sample the complete mask once on the device grid, like a directly
    // painted SVG with the same geometry. Round device coverage after sampling.
    Some(super::alpha_sampling::resample(&image, target_w, target_h).unwrap_or(image))
}

fn sample(bytes: &[u8], at4: usize, luminance: bool) -> f32 {
    if luminance {
        (bytes[at4] as f32 * 0.0722
            + bytes[at4 + 1] as f32 * 0.7152
            + bytes[at4 + 2] as f32 * 0.2126)
            * bytes[at4 + 3] as f32
            / (255.0 * 255.0)
    } else {
        bytes[at4 + 3] as f32 / 255.0
    }
}

fn combine(op: u8, a: f32, d: f32) -> f32 {
    match op {
        1 => a * (1.0 - d),
        2 => a * d,
        3 => a * (1.0 - d) + d * (1.0 - a),
        _ => a + d * (1.0 - a),
    }
}

/// Position inside the device-snapped copy covering device coordinate `p`
/// (copies start at `start + k * (tile + gap)`, both edges rounded), or
/// `None` in a gap or outside a single copy.
fn snapped_axis(p: f32, start: f32, tile: f32, gap: f32, once: bool) -> Option<f32> {
    if tile <= 0.0 {
        return None;
    }
    let period = tile + gap;
    let k0 = if once {
        0.0
    } else {
        ((p - start) / period).floor()
    };
    for k in [k0 - 1.0, k0, k0 + 1.0] {
        if once && k != 0.0 {
            continue;
        }
        let a = (start + k * period).round();
        let b = (start + k * period + tile).round();
        if p >= a && p < b {
            return Some((p - a) / (b - a));
        }
    }
    None
}
