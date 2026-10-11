//! Растровая маска и интервалы CSS shape-outside.

mod dilation;
pub(super) use dilation::dilate;

use super::{ShapeBox, svg_path_of};
use crate::paint::background::*;

/// Альфа-маска формы в холсте margin-box.
pub(super) fn shape_mask(raw: &str, b: &ShapeBox, cols: usize, rows: usize) -> Option<Vec<u8>> {
    let raw = raw.trim();
    // Круг и эллипс: вписанный эллипс опорной коробки (css-shapes-1 §3.1.1).
    // Горизонтальный путь сюда с ними не приходит — там они уходят в
    // `FloatShape::Ellipse` раньше; маска нужна вертикали, где форма
    // адресуется по блок-оси и аналитическим эллипсом не выражается.
    // Ветка стоит ПЕРВОЙ намеренно: `rrect_of` узнаёт слово-коробку, и
    // запись `circle(50% at left 40px top 40px) border-box` иначе стала бы
    // прямоугольником.
    if raw.contains("circle(") || raw.contains("ellipse(") {
        let at = raw.find("circle(").or_else(|| raw.find("ellipse("))?;
        let head = &raw[at..];
        let head = match head.find(')') {
            Some(end) => &head[..=end],
            None => head,
        };
        let (cx, cy, rx, ry) = shape_params(head, b.rw, b.rh, 1.0)?;
        let (cx, cy) = (cx + b.rx, cy + b.ry);
        if rx <= 0.0 || ry <= 0.0 {
            return Some(vec![0u8; cols * rows]);
        }
        let mut out = vec![0u8; cols * rows];
        for y in 0..rows {
            let dy = (y as f32 + 0.5 - cy) / ry;
            for x in 0..cols {
                let dx = (x as f32 + 0.5 - cx) / rx;
                if dx * dx + dy * dy <= 1.0 {
                    out[y * cols + x] = 255;
                }
            }
        }
        return Some(out);
    }
    // Скруглённый прямоугольник: inset/rect/xywh (+round) и слово-коробка
    // с её радиусами.
    if let Some(rect) = rrect_of(raw, b) {
        return Some(rrect_mask(rect.0, rect.1, cols, rows));
    }
    // Полигон / путь / shape(): готовым SVG-растеризатором — fill-rule и
    // антиалиас даром (как blink ExtractPathData).
    if let Some(d) = svg_path_of(raw, b) {
        let markup = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cols}" height="{rows}" viewBox="0 0 {cols} {rows}"><path d="{}" fill="#000000" fill-rule="{}"/></svg>"##,
            d.0, d.1
        );
        let img = crate::svg::raster::rasterize(&markup, cols as f32, rows as f32)?;
        let bytes = img.as_bytes(0)?;
        let sz = img.size(0);
        let (iw, ih) = (sz.width.0.max(1) as usize, sz.height.0.max(1) as usize);
        let mut out = vec![0u8; cols * rows];
        for y in 0..rows {
            let sy = (y * ih / rows).min(ih - 1);
            for x in 0..cols {
                let sx = (x * iw / cols).min(iw - 1);
                out[y * cols + x] = bytes[(sy * iw + sx) * 4 + 3];
            }
        }
        return Some(out);
    }
    // Картинка или градиент: альфа в content-box.
    if raw.contains("url(") || raw.contains("-gradient(") {
        let src = if raw.contains("-gradient(") {
            let at = raw.find("-gradient(")?;
            // Начало записи ищется от её ИМЕНИ: у `repeating-linear-gradient`
            // перед `-gradient(` дефис, а не пробел, и обрезка по пробелу
            // отдавала растеризатору всю строку целиком.
            let head = raw[..at]
                .rfind(|c: char| c.is_whitespace())
                .map_or(0, |s| s + 1);
            let start = raw[head..at].rfind("repeating-").map_or(head, |s| head + s);
            crate::paint::background::source(raw[start..].trim().trim_end_matches(|c| c != ')'))
                .and_then(|s| s.raster((b.cw.max(1.0), b.ch.max(1.0))))
        } else {
            crate::style::computed::parse_url(raw).and_then(|u| load(&u))
        }?;
        let bytes = src.as_bytes(0)?;
        let sz = src.size(0);
        let (iw, ih) = (sz.width.0.max(1) as usize, sz.height.0.max(1) as usize);
        let mut out = vec![0u8; cols * rows];
        for y in 0..rows {
            let fy = y as f32 - b.cy;
            if fy < 0.0 || fy >= b.ch {
                continue;
            }
            let sy = ((fy / b.ch.max(1.0)) * ih as f32) as usize;
            let sy = sy.min(ih - 1);
            for x in 0..cols {
                let fx = x as f32 - b.cx;
                if fx < 0.0 || fx >= b.cw {
                    continue;
                }
                let sx = ((fx / b.cw.max(1.0)) * iw as f32) as usize;
                out[y * cols + x] = bytes[(sy * iw + sx.min(iw - 1)) * 4 + 3];
            }
        }
        return Some(out);
    }
    None
}

/// Скруглённый прямоугольник в маску: SDF по угловым эллипсам (та же
/// математика, что `rasterize_rrect`, но с началом и размером).
pub(super) fn rrect_mask(
    rect: (f32, f32, f32, f32),
    radii: [(f32, f32); 4],
    cols: usize,
    rows: usize,
) -> Vec<u8> {
    let (x0, y0, w, h) = rect;
    let (x1, y1) = (x0 + w, y0 + h);
    // Переполнение радиусов: один множитель от худшей пары смежных
    // (css-backgrounds-3 §5.5).
    let mut k = 1.0f32;
    let sum = |a: f32, c: f32, side: f32| {
        if a + c > side && a + c > 0.0 {
            side / (a + c)
        } else {
            1.0
        }
    };
    k = k.min(sum(radii[0].0, radii[1].0, w));
    k = k.min(sum(radii[3].0, radii[2].0, w));
    k = k.min(sum(radii[0].1, radii[3].1, h));
    k = k.min(sum(radii[1].1, radii[2].1, h));
    let r: Vec<(f32, f32)> = radii.iter().map(|(a, c)| (a * k, c * k)).collect();
    let mut out = vec![0u8; cols * rows];
    for y in 0..rows {
        let fy = y as f32 + 0.5;
        if fy < y0 || fy > y1 {
            continue;
        }
        for x in 0..cols {
            let fx = x as f32 + 0.5;
            if fx < x0 || fx > x1 {
                continue;
            }
            // Углы: попадание в угловую четверть проверяется эллипсом.
            let inside = corner_ok(fx, fy, x0, y0, x1, y1, &r);
            if inside {
                out[y * cols + x] = 255;
            }
        }
    }
    out
}

pub(super) fn corner_ok(
    fx: f32,
    fy: f32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    r: &[(f32, f32)],
) -> bool {
    let check = |cx: f32, cy: f32, rx: f32, ry: f32| -> bool {
        if rx <= 0.0 || ry <= 0.0 {
            return true;
        }
        let (dx, dy) = ((fx - cx) / rx, (fy - cy) / ry);
        dx * dx + dy * dy <= 1.0
    };
    // tl
    if fx < x0 + r[0].0 && fy < y0 + r[0].1 && !check(x0 + r[0].0, y0 + r[0].1, r[0].0, r[0].1) {
        return false;
    }
    // tr
    if fx > x1 - r[1].0 && fy < y0 + r[1].1 && !check(x1 - r[1].0, y0 + r[1].1, r[1].0, r[1].1) {
        return false;
    }
    // br
    if fx > x1 - r[2].0 && fy > y1 - r[2].1 && !check(x1 - r[2].0, y1 - r[2].1, r[2].0, r[2].1) {
        return false;
    }
    // bl
    if fx < x0 + r[3].0 && fy > y1 - r[3].1 && !check(x0 + r[3].0, y1 - r[3].1, r[3].0, r[3].1) {
        return false;
    }
    true
}

/// Интервалы строк: от первого до последнего пикселя с альфой ВЫШЕ порога
/// (строго; дыры внутри строки заполняются — как blink).
pub(super) fn mask_intervals(
    mask: &[u8],
    cols: usize,
    rows: usize,
    thr: f32,
) -> Vec<Option<(i32, i32)>> {
    let t = (thr.clamp(0.0, 1.0) * 255.0) as u8;
    (0..rows)
        .map(|y| {
            let row = &mask[y * cols..(y + 1) * cols];
            let first = row.iter().position(|a| *a > t)?;
            let last = row.iter().rposition(|a| *a > t)?;
            Some((first as i32, last as i32 + 1))
        })
        .collect()
}
