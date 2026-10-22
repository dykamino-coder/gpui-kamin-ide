//! Растр слоёв-изображений: cross-fade, градиенты, раскладка стопов.

use super::*;
use crate::document::state::ValueCx;
use crate::resources::images::types::CpuRaster;
use std::sync::Arc;

/// Premultiplied cross-fade over the computed operand weights.
// Pixel channel indices address the source and accumulator buffers together.
pub(super) fn rasterize_cross_fade(
    src: &crate::style::computed::ImageSource,
    w: u32,
    h: u32,
    vcx: ValueCx<'_>,
) -> Option<Arc<CpuRaster>> {
    let crate::style::computed::ImageSource::CrossFade(operands) = src else {
        return None;
    };
    let n = (w * h) as usize;
    let mut items = Vec::new();
    for (operand, weight) in operands {
        let image =
            sources::computed_source(operand, true, vcx)?.raster((w as f32, h as f32), vcx)?;
        let (iw, ih) = (image.width.max(1), image.height.max(1));
        let bytes = &image.bytes;
        let mut out = Vec::with_capacity(n * 4);
        for y in 0..h {
            let sy = (y * ih / h.max(1)).min(ih - 1);
            for x in 0..w {
                let sx = (x * iw / w.max(1)).min(iw - 1);
                let at = ((sy * iw + sx) * 4) as usize;
                out.extend_from_slice(bytes.get(at..at + 4)?);
            }
        }
        items.push((*weight, out));
    }
    // Точки слоёв и результата — с ПРЯМОЙ альфой (так их отдаёт растеризатор
    // и так их ждёт отрисовка плитки); сумма — в премультиплицированных
    // (css-images-4 §2.6), назад к прямой — делением на итоговую альфу.
    // Без премультипликации полупрозрачный красный 1% тянул смесь к красному
    // (`cross-fade-premultiplied-alpha`), а запись в премультиплицированных
    // темнила итог дважды (`cross-fade-target-alpha`).
    let mut out = vec![0u8; n * 4];
    for p in 0..n {
        let (mut acc, mut alpha) = ([0.0f32; 3], 0.0f32);
        for (wt, bytes) in &items {
            let a = bytes[p * 4 + 3] as f32 / 255.0 * wt;
            for ch in 0..3 {
                acc[ch] += bytes[p * 4 + ch] as f32 * a;
            }
            alpha += a;
        }
        for ch in 0..3 {
            out[p * 4 + ch] = if alpha > 0.0 {
                (acc[ch] / alpha).round().clamp(0.0, 255.0) as u8
            } else {
                0
            };
        }
        out[p * 4 + 3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    crate::resources::images::pixels::bgra(w, h, out)
}

pub(super) fn rasterize_gradient(
    src: &crate::style::computed::ImageSource,
    w: u32,
    h: u32,
    vcx: ValueCx<'_>,
) -> Option<Arc<CpuRaster>> {
    use crate::style::computed::ImageSource;
    match src {
        ImageSource::Color(color) => {
            let c = color.value;
            let pixel = [
                (c.b * 255.0).round() as u8,
                (c.g * 255.0).round() as u8,
                (c.r * 255.0).round() as u8,
                (c.a * 255.0).round() as u8,
            ];
            crate::resources::images::pixels::bgra(
                w,
                h,
                pixel
                    .into_iter()
                    .cycle()
                    .take((w * h * 4) as usize)
                    .collect(),
            )
        }
        ImageSource::CrossFade(_) => rasterize_cross_fade(src, w, h, vcx),
        ImageSource::Conic(g) => conic::rasterize(g, (w as f32, h as f32), 1.0, vcx),
        ImageSource::Gradient { .. } => {
            gradient_raster::raster(src, w, h, (w as f32, h as f32), false, vcx)
        }
        _ => sources::computed_source(src, true, vcx)?.raster((w as f32, h as f32), vcx),
    }
}

pub(crate) use crate::style::computed::place_stops;

/// Доля точки в ПОВТОРЯЮЩЕМСЯ градиенте (css-images-3 §3.6): узор стопов
/// повторяется бесконечно в обе стороны со сдвигом на разность позиций
/// последнего и первого стопа. При нулевой разности повторять нечем:
/// вырожденный повтор красит всю картинку цветом последнего стопа.
pub(super) fn wrap_repeat(t: f32, stops: &[(crate::style::values::value::Color, f32)]) -> f32 {
    let (Some(first), Some(last)) = (stops.first(), stops.last()) else {
        return t;
    };
    let span = last.1 - first.1;
    if span <= 0.0 {
        // CSS Images 3 §3.4: coincident repeating stops produce the last
        // color everywhere, including when their common position is >100%.
        // Blink CSSGradientValue::NormalizeAndAddStops uses the same rule.
        return last.1.next_up();
    }
    first.1 + (t - first.1).rem_euclid(span)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedural_images_keep_bgra_and_premultiplied_mixing_goldens() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        for (css, expected) in [
            (
                "cross-fade(rgba(255, 0, 0, .5) 50%, blue 50%)",
                [170, 0, 85, 192],
            ),
            ("filter(rgb(0, 128, 0), invert(1))", [255, 127, 255, 255]),
            ("linear-gradient(red, red)", [0, 0, 255, 255]),
            ("radial-gradient(red, red)", [0, 0, 255, 255]),
            ("conic-gradient(red, red)", [0, 0, 255, 255]),
        ] {
            let source = crate::style::computed::ImageSource::parse(css, vcx).unwrap();
            for (w, h) in [(1, 1), (7, 5), (16, 9)] {
                let raster = rasterize_gradient(&source, w, h, vcx).unwrap();
                assert_eq!((raster.width, raster.height), (w, h), "{css}");
                for pixel in raster.bytes.chunks_exact(4) {
                    assert_eq!(pixel, expected, "{css}");
                }
            }
        }
    }
}
