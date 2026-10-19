//! Растр слоёв-изображений: cross-fade, градиенты, раскладка стопов.

use crate::document::state::ValueCx;
use crate::paint::background::*;
use gpui::RenderImage;
use std::sync::Arc;

/// Задать рисунку область просмотра размером с плитку.
///
/// Область просмотра фонового рисунка — это его ПЛИТКА, а не что-то своё:
/// доли внутри рисунка (`height="50%"`, `<rect width="100%">`) считаются от
/// неё. Растеризатор же разбирает разметку как отдельный документ, и рисунок
/// с долевым размером корня не имеет для него размера вовсе — выходил пустой
/// растр, а с ним и пустая страница (вся папка `background-size/vector`).
/// Поэтому свои `width`/`height` корня заменяются размером плитки.
pub(super) fn with_viewport(markup: &str, tile: (f32, f32)) -> String {
    let Some(open) = markup.find("<svg") else {
        return markup.to_string();
    };
    let Some(close) = markup[open..].find('>').map(|e| open + e) else {
        return markup.to_string();
    };
    let mut head = markup[open + 4..close].to_string();
    let mut own = [None::<String>, None::<String>];
    for (i, name) in ["width", "height"].iter().enumerate() {
        while let Some(at) = head.find(&format!("{name}=")) {
            let rest = &head[at + name.len() + 1..];
            let Some(quote) = rest.chars().next() else {
                break;
            };
            let Some(end) = rest[1..].find(quote) else {
                break;
            };
            own[i] = Some(rest[1..1 + end].to_string());
            head.replace_range(at..at + name.len() + 2 + end + 1, "");
        }
    }
    // Без viewBox содержимое НЕ растёт под новый вьюпорт: рисунок 50x50 в
    // плитке 100x100 занимал четверть, а маска-плитка выходила с прозрачными
    // полосами (mask-repeat-1, mask-size-cover). Свои размеры рута становятся
    // рамкой просмотра — содержимое масштабируется, как в браузере.
    if !head.contains("viewBox")
        && let (Some(w), Some(h)) = (&own[0], &own[1])
    {
        let plain = |v: &str| v.trim().trim_end_matches("px").parse::<f32>().ok();
        if let (Some(w), Some(h)) = (plain(w), plain(h)) {
            head.push_str(&format!(" viewBox=\"0 0 {w} {h}\""));
        }
    }
    format!(
        "{}<svg width=\"{}\" height=\"{}\"{head}>{}",
        &markup[..open],
        tile.0,
        tile.1,
        &markup[close + 1..]
    )
}

/// Premultiplied cross-fade over the computed operand weights.
// Pixel channel indices address the source and accumulator buffers together.
#[allow(clippy::needless_range_loop)]
pub(super) fn rasterize_cross_fade(
    src: &crate::style::computed::ImageSource,
    w: u32,
    h: u32,
    vcx: ValueCx<'_>,
) -> Option<Arc<RenderImage>> {
    let crate::style::computed::ImageSource::CrossFade(operands) = src else {
        return None;
    };
    let n = (w * h) as usize;
    let mut items = Vec::new();
    for (operand, weight) in operands {
        let image = sources::computed_source(operand, true)?.raster((w as f32, h as f32), vcx)?;
        let size = image.size(0);
        let (iw, ih) = (size.width.0.max(1) as u32, size.height.0.max(1) as u32);
        let bytes = image.as_bytes(0)?;
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
    gpui::bgra_bytes_to_image(w, h, out)
}

pub(super) fn rasterize_gradient(
    src: &crate::style::computed::ImageSource,
    w: u32,
    h: u32,
    vcx: ValueCx<'_>,
) -> Option<Arc<RenderImage>> {
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
            gpui::bgra_bytes_to_image(
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
        _ => sources::computed_source(src, true)?.raster((w as f32, h as f32), vcx),
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
