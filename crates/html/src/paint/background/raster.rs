//! Растр слоёв-изображений: cross-fade, градиенты, раскладка стопов.

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
    if !head.contains("viewBox") {
        if let (Some(w), Some(h)) = (&own[0], &own[1]) {
            let plain = |v: &str| v.trim().trim_end_matches("px").parse::<f32>().ok();
            if let (Some(w), Some(h)) = (plain(w), plain(h)) {
                head.push_str(&format!(" viewBox=\"0 0 {w} {h}\""));
            }
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

/// Градиент как источник картинки: считается по своей формуле в растр 64×64.
///
/// Для рамки-картинки соотношение сторон источника роли не играет — куски всё
/// равно растягиваются под свои места, поэтому мелкого растра достаточно.
/// Линейный и конический считаются честно; радиальный отдаёт осевой ход
/// цвета — девятке рамки радиальной решётки и не нужно.
/// `cross-fade(<cf-image>#)` (css-images-4 §2.6, `csswg-drafts/css-images-4/
/// Overview.bs` «cross-fade»): взвешенная сумма картинок в
/// премультиплицированных цветах. Доли: без своей доли картинка делит
/// остаток до 100% поровну с такими же; сумма больше 100% нормируется к 100%;
/// меньше — результат частично прозрачен. Слагаемое — цвет (или
/// `image(<color>)`), градиент или `url()`; растр приводится к размеру плитки
/// ближайшей точкой.
pub(super) fn rasterize_cross_fade(src: &str, w: u32, h: u32) -> Option<Arc<RenderImage>> {
    let inner = src.strip_prefix("cross-fade(")?;
    let inner = &inner[..inner.rfind(')')?];
    let n = (w * h) as usize;
    let mut items: Vec<(Option<f32>, Vec<u8>)> = vec![];
    for part in crate::style::css::split_args(inner) {
        let mut pct = None;
        let mut img = None;
        for t in split_top(part.trim()) {
            match t.strip_suffix('%').and_then(|v| v.parse::<f32>().ok()) {
                Some(p) => pct = Some((p / 100.0).clamp(0.0, 1.0)),
                None => img = Some(t),
            }
        }
        let img = img?;
        let colour = crate::style::values::value::Color::parse(
            img.strip_prefix("image(").and_then(|t| t.strip_suffix(')')).unwrap_or(img),
        );
        let buf = if let Some(c) = colour {
            let px = [
                (c.b * 255.0).round() as u8,
                (c.g * 255.0).round() as u8,
                (c.r * 255.0).round() as u8,
                (c.a * 255.0).round() as u8,
            ];
            px.iter().copied().cycle().take(n * 4).collect()
        } else {
            let image = if img.contains("gradient(") {
                rasterize_gradient(img, w, h)?
            } else {
                let url = crate::style::computed::parse_url(img)?;
                source(&url)?.raster((w as f32, h as f32))?
            };
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
            out
        };
        items.push((pct, buf));
    }
    if items.is_empty() {
        return None;
    }
    let given: f32 = items.iter().filter_map(|i| i.0).sum();
    let free = items.iter().filter(|i| i.0.is_none()).count() as f32;
    let weights: Vec<f32> = items
        .iter()
        .map(|i| match i.0 {
            Some(p) if given > 1.0 => p / given,
            Some(p) => p,
            None if given >= 1.0 => 0.0,
            None => (1.0 - given) / free,
        })
        .collect();
    // Точки слоёв и результата — с ПРЯМОЙ альфой (так их отдаёт растеризатор
    // и так их ждёт отрисовка плитки); сумма — в премультиплицированных
    // (css-images-4 §2.6), назад к прямой — делением на итоговую альфу.
    // Без премультипликации полупрозрачный красный 1% тянул смесь к красному
    // (`cross-fade-premultiplied-alpha`), а запись в премультиплицированных
    // темнила итог дважды (`cross-fade-target-alpha`).
    let mut out = vec![0u8; n * 4];
    for p in 0..n {
        let (mut acc, mut alpha) = ([0.0f32; 3], 0.0f32);
        for (it, wt) in items.iter().zip(&weights) {
            let a = it.1[p * 4 + 3] as f32 / 255.0 * wt;
            for ch in 0..3 {
                acc[ch] += it.1[p * 4 + ch] as f32 * a;
            }
            alpha += a;
        }
        for ch in 0..3 {
            out[p * 4 + ch] = if alpha > 0.0 { (acc[ch] / alpha).round().clamp(0.0, 255.0) as u8 } else { 0 };
        }
        out[p * 4 + 3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    gpui::bgra_bytes_to_image(w, h, out)
}

pub(super) fn rasterize_gradient(src: &str, w: u32, h: u32) -> Option<Arc<RenderImage>> {
    if crate::style::computed::parse_image_color(src).is_some() {
        return sources::raster_color(src, w, h);
    }
    if src.starts_with("cross-fade(") {
        return rasterize_cross_fade(src, w, h);
    }
    if conic::is_conic(src) {
        return conic::rasterize(src, (w as f32, h as f32), 1.0);
    }
    gradient_raster::raster(src, w, h, (w as f32, h as f32), false)
}

/// Угол позиции стопа в долях оборота: `90deg`, `25%`, `0.25turn`, голый `0`.
pub(super) fn angle_fraction(token: &str) -> Option<f32> {
    let token = token.trim();
    if let Some(n) = token.strip_suffix('%') {
        return n.parse::<f32>().ok().map(|v| v / 100.0);
    }
    if let Some(n) = token.strip_suffix("deg") {
        return n.parse::<f32>().ok().map(|v| v / 360.0);
    }
    if let Some(n) = token.strip_suffix("grad") {
        return n.parse::<f32>().ok().map(|v| v / 400.0);
    }
    if let Some(n) = token.strip_suffix("rad") {
        return n.parse::<f32>().ok().map(|v| v / std::f32::consts::TAU);
    }
    if let Some(n) = token.strip_suffix("turn") {
        return n.parse::<f32>().ok();
    }
    // Ноль без единицы — законный угол в CSS; прочие голые числа — нет.
    (token == "0").then_some(0.0)
}

/// Расставить позиции стопов по правилам css-images: крайние без позиции — на
/// края, промежуточные — поровну между соседями с позициями, и позиции не
/// убывают.
pub(crate) fn place_stops(
    raw: Vec<(crate::style::values::value::Color, Option<f32>)>,
) -> Vec<(crate::style::values::value::Color, f32)> {
    let last = raw.len() - 1;
    let mut out: Vec<(crate::style::values::value::Color, f32)> = Vec::with_capacity(raw.len());
    // Зажим снизу — только позицией ПРЕДШЕСТВЕННИКА (css-images-3 §3.5.3):
    // у первого стопа его нет, и отрицательная позиция законна. Нулевой
    // пол сдвигал `calc(-65535000px)` в ноль, и вся коробка красилась
    // первым цветом (`gradient-eval-*`).
    let mut floor = f32::NEG_INFINITY;
    for (i, (colour, pos)) in raw.iter().enumerate() {
        let at = match pos {
            Some(v) => v.max(floor),
            None if i == 0 => 0.0,
            None if i == last => 1.0f32.max(floor),
            None => {
                // Доля до следующего стопа с позицией (или до конца).
                let (mut next, mut steps) = (1.0f32, (last - i + 1) as f32);
                for (j, (_, p)) in raw.iter().enumerate().skip(i + 1) {
                    if let Some(v) = p {
                        next = v.max(floor);
                        steps = (j - i + 1) as f32;
                        break;
                    }
                }
                floor + (next - floor) / steps
            }
        };
        floor = at;
        out.push((*colour, at));
    }
    out
}

/// Доля точки в ПОВТОРЯЮЩЕМСЯ градиенте (css-images-3 §3.6): узор стопов
/// повторяется бесконечно в обе стороны со сдвигом на разность позиций
/// последнего и первого стопа. Нулевая разность повторять нечем — спека
/// объявляет такой градиент вырожденным, и точка остаётся как есть.
pub(super) fn wrap_repeat(t: f32, stops: &[(crate::style::values::value::Color, f32)]) -> f32 {
    let (Some(first), Some(last)) = (stops.first(), stops.last()) else {
        return t;
    };
    let span = last.1 - first.1;
    if span <= 0.0 {
        return t;
    }
    first.1 + (t - first.1).rem_euclid(span)
}
