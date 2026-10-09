//! Профиль фигуры обтекания (shape-outside, shape-margin): маска, интервалы, расширение.

use crate::paint::background::*;

// --- Общий путь формы обтекания (css-shapes-1 §3, §shape-margin) ---------
//
// Форма растрируется альфа-маской в холст margin-box (1 пиксель = 1 точка,
// как blink RasterShape), маска сводится к интервалам строк «первый..
// последний непрозрачный», интервалы раздуваются диском shape-margin
// (дилатация Минковского по blink ComputeShapeMarginIntervals) и
// превращаются в экстенты от начала стороны текста. Клип к margin-box
// двойной: холст режет форму, зажим на шаге экстента режет поле
// («a shape can only ever reduce a float area»).

/// Геометрия флоата в системе его margin-box, всё в CSS-точках.
pub struct ShapeBox {
    pub mw: f32,
    pub mh: f32,
    /// Опорная коробка формы.
    pub rx: f32,
    pub ry: f32,
    pub rw: f32,
    pub rh: f32,
    /// Content-box (для картинки/градиента).
    pub cx: f32,
    pub cy: f32,
    pub cw: f32,
    pub ch: f32,
    /// Радиусы опорной коробки (tl,tr,br,bl), эллиптические.
    pub radius: [(f32, f32); 4],
    pub threshold: f32,
}

/// Экстенты обтекания по строкам margin-box; None — форма нераспознана
/// (вызывающий откатывается к прямоугольнику коробки). Пустая форма —
/// нули: «empty float area», НЕ фоллбек.
pub fn shape_profile(raw: &str, b: &ShapeBox, sm: f32, side: i32) -> Option<Vec<f32>> {
    let rows = b.mh.ceil().max(1.0) as usize;
    let cols = b.mw.ceil().max(1.0) as usize;
    let mask = shape_mask(raw, b, cols, rows)?;
    let mut iv = mask_intervals(&mask, cols, rows, b.threshold);
    // `shape-margin` раздувает фигуру наружу на своё расстояние
    // (css-shapes-1 §2.2): контур обтекания — множество точек не дальше
    // `shape-margin` от исходной фигуры. Раздутие было написано и не
    // подключено — общий растровый путь отдавал профиль как есть.
    dilate(&mut iv, sm, cols, rows);
    Some(
        iv.into_iter()
            .map(|slot| match slot {
                None => 0.0,
                Some((x1, x2)) => {
                    if side < 0 {
                        (x2 as f32).clamp(0.0, b.mw)
                    } else {
                        b.mw - (x1 as f32).clamp(0.0, b.mw)
                    }
                }
            })
            .collect(),
    )
}

/// Экстенты обтекания вдоль БЛОК-оси вертикального письма.
///
/// В вертикали строки набора — это колонки: блок-ось горизонтальна и идёт
/// от правого края (`vertical-rl`, `sideways-rl`), инлайн-ось вертикальна,
/// а line-left = верх, line-right = низ (css-writing-modes-4 §6.3, таблица
/// logical-to-physical). Поэтому ту же маску формы надо резать СТОЛБЦАМИ, а
/// экстент мерить вдоль физической вертикали. Индекс результата —
/// расстояние от блок-старта margin-box, значение — экстент от своей
/// line-стороны; ровно в этих осях работает `FlowRow::vertical_rl`.
///
/// Раздутие `shape-margin` — тот же диск Минковского (css-shapes-1 §2.2),
/// что и у строчного профиля: `dilate` не знает, какая ось «длинная», ей
/// достаточно поменять местами два размера.
pub fn shape_profile_block(raw: &str, b: &ShapeBox, sm: f32, side: i32) -> Option<Vec<f32>> {
    let rows = b.mh.ceil().max(1.0) as usize;
    let cols = b.mw.ceil().max(1.0) as usize;
    // Circle/ellipse without shape-margin: exact extent per one-pixel column,
    // i.e. the chord at the column edge nearest the centre — a line band
    // takes the shape's maximum over its whole block range (css-shapes-1
    // §2), and pixel-centre sampling fell half a pixel short
    // (`shape-outside-circle-048..053`: a box one device row high).
    if sm <= 0.0 && (raw.contains("circle(") || raw.contains("ellipse(")) {
        let raw = raw.trim();
        let at = raw.find("circle(").or_else(|| raw.find("ellipse("))?;
        let head = &raw[at..];
        let head = match head.find(')') {
            Some(end) => &head[..=end],
            None => head,
        };
        let (cx, cy, rx, ry) = shape_params(head, b.rw, b.rh, 1.0)?;
        let (cx, cy) = (cx + b.rx, cy + b.ry);
        return Some(
            (0..cols)
                .rev()
                .map(|x| {
                    if rx <= 0.0 || ry <= 0.0 {
                        return 0.0;
                    }
                    let (x0, x1) = (x as f32, x as f32 + 1.0);
                    let dx = if cx < x0 {
                        x0 - cx
                    } else if cx > x1 {
                        cx - x1
                    } else {
                        0.0
                    };
                    if dx >= rx {
                        return 0.0;
                    }
                    let h = ry * (1.0 - (dx / rx) * (dx / rx)).sqrt();
                    if side < 0 {
                        (cy + h).clamp(0.0, b.mh)
                    } else {
                        b.mh - (cy - h).clamp(0.0, b.mh)
                    }
                })
                .collect(),
        );
    }
    let mask = shape_mask(raw, b, cols, rows)?;
    let t = (b.threshold.clamp(0.0, 1.0) * 255.0) as u8;
    let mut iv: Vec<Option<(i32, i32)>> = (0..cols)
        .map(|x| {
            let first = (0..rows).find(|&y| mask[y * cols + x] > t)?;
            let last = (0..rows).rev().find(|&y| mask[y * cols + x] > t)?;
            Some((first as i32, last as i32 + 1))
        })
        .collect();
    dilate(&mut iv, sm, rows, cols);
    Some(
        (0..cols)
            .rev()
            .map(|x| match iv[x] {
                None => 0.0,
                Some((y1, y2)) => {
                    if side < 0 {
                        (y2 as f32).clamp(0.0, b.mh)
                    } else {
                        b.mh - (y1 as f32).clamp(0.0, b.mh)
                    }
                }
            })
            .collect(),
    )
}

/// Альфа-маска формы в холсте margin-box.
fn shape_mask(raw: &str, b: &ShapeBox, cols: usize, rows: usize) -> Option<Vec<u8>> {
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
        let img = crate::svg::rasterize(&markup, cols as f32, rows as f32)?;
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

/// Контур для SVG-растеризатора: polygon / path / shape.
pub(super) fn svg_path_of(raw: &str, b: &ShapeBox) -> Option<(String, &'static str)> {
    let raw = raw.trim();
    // Функция может идти ПОСЛЕ слова-коробки: `padding-box polygon(...)`.
    if let Some(at) = raw.find("polygon(") {
        let inner = raw[at + 8..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 8..]);
        {
            let mut rule = "nonzero";
            let mut pts_src = inner;
            if let Some(rest) = inner.trim_start().strip_prefix("evenodd") {
                rule = "evenodd";
                pts_src = rest.trim_start().trim_start_matches(',');
            } else if let Some(rest) = inner.trim_start().strip_prefix("nonzero") {
                pts_src = rest.trim_start().trim_start_matches(',');
            }
            let len_px = |t: &str, base: f32| -> f32 {
                match crate::style::values::value::Len::parse(t) {
                    Some(crate::style::values::value::Len::Px(v)) => v,
                    Some(crate::style::values::value::Len::Pct(k)) => k * base,
                    _ => 0.0,
                }
            };
            let mut d = String::new();
            for (i, pair) in pts_src.split(',').enumerate() {
                let mut it = pair.split_whitespace();
                let x = b.rx + len_px(it.next()?, b.rw);
                let y = b.ry + len_px(it.next()?, b.rh);
                d.push_str(if i == 0 { "M" } else { "L" });
                d.push_str(&format!("{x} {y} "));
            }
            if d.is_empty() {
                return None;
            }
            d.push('Z');
            return Some((
                d,
                if rule == "evenodd" {
                    "evenodd"
                } else {
                    "nonzero"
                },
            ));
        }
    }
    if let Some(at) = raw.find("path(") {
        let inner = raw[at + 5..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 5..]);
        {
            // `path( [<fill-rule>,]? <string> )` — css-shapes-1 §3.1:
            // правило намотки стоит ПЕРЕД строкой контура и отделено
            // запятой. Оно не отрезалось, и слово `evenodd` вместе с
            // запятой уезжало в атрибут `d` — контур не разбирался вовсе.
            let mut rule = "nonzero";
            let mut body = inner.trim();
            if let Some(rest) = body.strip_prefix("evenodd") {
                rule = "evenodd";
                body = rest.trim_start().trim_start_matches(',').trim_start();
            } else if let Some(rest) = body.strip_prefix("nonzero") {
                body = rest.trim_start().trim_start_matches(',').trim_start();
            }
            let d = body.trim_matches('"').trim_matches('\'').to_string();
            if d.is_empty() {
                return None;
            }
            return Some((d, rule));
        }
    }
    if let Some(at) = raw.find("shape(") {
        let inner = raw[at + 6..]
            .rsplit_once(')')
            .map(|(a, _)| a)
            .unwrap_or(&raw[at + 6..]);
        {
            let d = shape_to_path(&inner.replace(',', ";"), b.rw, b.rh)?;
            return Some((d, "nonzero"));
        }
    }
    None
}

/// Скруглённый прямоугольник в маску: SDF по угловым эллипсам (та же
/// математика, что `rasterize_rrect`, но с началом и размером).
fn rrect_mask(
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

fn corner_ok(fx: f32, fy: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: &[(f32, f32)]) -> bool {
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
fn mask_intervals(mask: &[u8], cols: usize, rows: usize, thr: f32) -> Vec<Option<(i32, i32)>> {
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

/// Дилатация Минковского диском `sm` (blink ComputeShapeMarginIntervals):
/// каждый интервал раздаётся соседним строкам с сужением по дуге; ранний
/// выход, когда сосед и так шире. Вертикаль жёстко в [0, rows).
fn dilate(iv: &mut [Option<(i32, i32)>], sm: f32, cols: usize, rows: usize) {
    if sm <= 0.0 {
        return;
    }
    let cap = ((cols.max(rows) as f32) * std::f32::consts::SQRT_2) as i32;
    let r = (sm.ceil() as i32).clamp(0, cap.max(1));
    let dx: Vec<i32> = (0..=r)
        .map(|k| (((r * r - k * k) as f32).sqrt()) as i32)
        .collect();
    let src: Vec<Option<(i32, i32)>> = iv.to_vec();
    let top = src.iter().position(|s| s.is_some());
    let bot = src.iter().rposition(|s| s.is_some());
    let (top, bot) = match (top, bot) {
        (Some(a), Some(b)) => (a as i32, b as i32),
        _ => return,
    };
    let unite = |slot: &mut Option<(i32, i32)>, x1: i32, x2: i32| match slot {
        None => *slot = Some((x1, x2)),
        Some((a, b)) => {
            *a = (*a).min(x1);
            *b = (*b).max(x2);
        }
    };
    for y in 0..rows as i32 {
        let Some((x1, x2)) = src[y as usize] else {
            continue;
        };
        let contains =
            |m: i32| -> bool { matches!(src[m as usize], Some((a, b)) if a <= x1 && b >= x2) };
        // вверх
        let y0 = (y - r).max(0);
        let mut my = y - 1;
        while my >= y0 {
            if my > top && contains(my) {
                break;
            }
            let d = dx[(y - my) as usize];
            unite(&mut iv[my as usize], x1 - d, x2 + d);
            my -= 1;
        }
        unite(&mut iv[y as usize], x1 - dx[0], x2 + dx[0]);
        // вниз
        let y1 = (y + r).min(rows as i32 - 1);
        let mut my = y + 1;
        while my <= y1 {
            if my < bot && contains(my) {
                break;
            }
            let d = dx[(my - y) as usize];
            unite(&mut iv[my as usize], x1 - d, x2 + d);
            my += 1;
        }
    }
}
