//! CSS `clip-path` на детях `<svg>`: синтез `<clipPath>` и опорные коробки фигур.

use super::serialize::escape_attr;
use super::{IN_CLIP, VIEW_BOX};
use crate::dom::{Element, Node};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Сдвиг `transform="translate(x[ ,]y)"` — единственный вид, который переносит
/// рамку ребёнка в систему группы без поворота и масштаба.
pub(super) fn translate_only(t: &str) -> Option<(f32, f32)> {
    let inner = t.trim().strip_prefix("translate(")?.strip_suffix(')')?;
    let v: Vec<f32> = inner
        .split([' ', ','])
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<f32>().ok())
        .collect::<Option<_>>()?;
    match v.as_slice() {
        [x] => Some((*x, 0.0)),
        [x, y] => Some((*x, *y)),
        _ => None,
    }
}

/// Половина обводки фигуры — на столько stroke-box шире fill-box
/// (css-masking-1: «stroke bounding box»). Нет обводки — ноль.
fn stroke_half(e: &Element) -> f32 {
    let paint = e
        .attr("stroke")
        .map(str::to_string)
        .or_else(|| e.style.svg_stroke.clone());
    if paint.as_deref().is_none_or(|s| s.trim() == "none") {
        return 0.0;
    }
    e.attr("stroke-width")
        .map(str::to_string)
        .or_else(|| e.style.svg_stroke_width.clone())
        .and_then(|w| w.trim().trim_end_matches("px").parse::<f32>().ok())
        .unwrap_or(1.0)
        * 0.5
}

/// Рамка фигуры в её пользовательской системе: fill-box (SVG 2 «object
/// bounding box»), при `stroke` — stroke-box. Только фигуры с явной
/// геометрией и группы из них; остальное — `None`, синтеза обрезки нет.
pub(super) fn shape_box(e: &Element, stroke: bool) -> Option<(f32, f32, f32, f32)> {
    let num = |k: &str| {
        e.attr(k)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
    };
    let css = |l: Option<crate::style::values::value::Len>| match l {
        Some(crate::style::values::value::Len::Px(v)) => Some(v),
        _ => None,
    };
    let (x, y, w, h) = match e.tag.to_ascii_lowercase().as_str() {
        "rect" | "image" => (
            num("x").or(css(e.style.svg_x)).unwrap_or(0.0),
            num("y").or(css(e.style.svg_y)).unwrap_or(0.0),
            num("width").or(css(e.style.width))?,
            num("height").or(css(e.style.height))?,
        ),
        "circle" => {
            let r = num("r")?;
            let (cx, cy) = (num("cx").unwrap_or(0.0), num("cy").unwrap_or(0.0));
            (cx - r, cy - r, 2.0 * r, 2.0 * r)
        }
        "ellipse" => {
            let (rx, ry) = (num("rx")?, num("ry")?);
            let (cx, cy) = (num("cx").unwrap_or(0.0), num("cy").unwrap_or(0.0));
            (cx - rx, cy - ry, 2.0 * rx, 2.0 * ry)
        }
        "g" => {
            // Объединение рамок детей в системе группы (clip-path-path-003:
            // `<g>` из двух прямоугольников, начало рамки — (0,-100)).
            let mut acc: Option<(f32, f32, f32, f32)> = None;
            for n in &e.children {
                let Node::Element(c) = n else { continue };
                if c.style.transform.is_some() || c.style.translate.is_some() {
                    return None;
                }
                let (dx, dy) = match c.attr("transform") {
                    Some(t) => translate_only(t)?,
                    None => (0.0, 0.0),
                };
                let (cx, cy, cw, ch) = shape_box(c, stroke)?;
                let (x0, y0) = (cx + dx, cy + dy);
                let (x1, y1) = (x0 + cw, y0 + ch);
                acc = Some(match acc {
                    None => (x0, y0, x1, y1),
                    Some((a, b, c1, d)) => (a.min(x0), b.min(y0), c1.max(x1), d.max(y1)),
                });
            }
            let (x0, y0, x1, y1) = acc?;
            return Some((x0, y0, x1 - x0, y1 - y0));
        }
        _ => return None,
    };
    let half = if stroke { stroke_half(e) } else { 0.0 };
    Some((x - half, y - half, w + 2.0 * half, h + 2.0 * half))
}

/// Конец функции `name(...)` с учётом вложенных скобок: индекс ПОСЛЕ ее `)`.
fn func_end(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Содержимое `<clipPath>` для CSS-фигуры в пользовательской системе фигуры;
/// `rb` — опорная коробка (x, y, w, h). `None` — фигура не выражается.
fn clip_body(
    c: &crate::style::computed::Computed,
    (bx, by, bw, bh): (f32, f32, f32, f32),
) -> Option<String> {
    use crate::style::values::value::Len;
    let at = |l: Len, side: f32| match l {
        Len::Px(v) => Some(v),
        Len::Pct(p) => Some(p * side),
        _ => None,
    };
    if let Some(points) = &c.clip_polygon {
        let mut pts = String::new();
        for (x, y) in points {
            pts.push_str(&format!("{},{} ", bx + at(*x, bw)?, by + at(*y, bh)?));
        }
        let rule = if c.clip_polygon_evenodd {
            "evenodd"
        } else {
            "nonzero"
        };
        return Some(format!(
            "<polygon clip-rule=\"{rule}\" points=\"{}\"/>",
            pts.trim_end()
        ));
    }
    if let Some([t, r, b, l]) = c.clip_inset {
        let (t, r, b, l) = (at(t, bh)?, at(r, bw)?, at(b, bh)?, at(l, bw)?);
        let (w, h) = ((bw - l - r).max(0.0), (bh - t - b).max(0.0));
        let round = c.clip_round.unwrap_or(0.0).max(0.0);
        return Some(format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{w}\" height=\"{h}\" rx=\"{round}\" ry=\"{round}\"/>",
            bx + l,
            by + t
        ));
    }
    if c.clip_bare_box {
        return Some(format!(
            "<rect x=\"{bx}\" y=\"{by}\" width=\"{bw}\" height=\"{bh}\"/>"
        ));
    }
    let spec = c.clip_shape.as_deref()?;
    if let Some(raw) = spec.strip_prefix("shape:") {
        if !(raw.starts_with("circle(") || raw.starts_with("ellipse(")) {
            return None;
        }
        // Хвост после функции — слово коробки (`… view-box`): отрезается.
        let func = &raw[..func_end(raw)?];
        let (cx, cy, rx, ry) = crate::paint::background::shape_params(func, bw, bh, 1.0)?;
        return Some(format!(
            "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{rx}\" ry=\"{ry}\"/>",
            bx + cx,
            by + cy
        ));
    }
    let (rule, d) = if let Some(rest) = spec.strip_prefix("pathdef:") {
        let (rule, d) = rest.split_once(':')?;
        (rule, d.to_string())
    } else if let Some(rest) = spec.strip_prefix("shapedef:") {
        let (rule, body) = rest.split_once(':')?;
        (rule, crate::paint::background::shape_to_path(body, bw, bh)?)
    } else {
        return None;
    };
    // Точки `path()`/`shape()` отсчитываются от НАЧАЛА опорной коробки.
    let mut path = format!("<path clip-rule=\"{rule}\" transform=\"translate({bx} {by})\" d=\"");
    escape_attr(&d, &mut path);
    path.push_str("\"/>");
    Some(path)
}

/// CSS-обрезка базовой фигурой на SVG-ребёнке (css-masking-1 §5.1): usvg
/// понимает у `clip-path` только `url()`, поэтому фигура синтезируется
/// `<clipPath clipPathUnits="userSpaceOnUse">` ПЕРЕД элементом, а элемент
/// получает ссылку. Возвращает id синтезированного определения.
///
/// Прежде запись молча терялась, и фигура рисовалась целиком
/// (`svg-clip-path-fixed-values` 4.22, `clip-path-path-003` 1.05,
/// `svg-clip-path-ellipse-offset` 0.82, `clip-path-viewBox-1a/1b` 2.19/6.72;
/// близнецы `*-borderBox-1b`, `*-strokeBox-1b/1c` и родня держались под
/// порогом случайно — 0.47-0.49).
pub(super) fn synth_clip(e: &Element, out: &mut String) -> Option<String> {
    if e.tag.eq_ignore_ascii_case("svg") || IN_CLIP.with(|c| c.get()) {
        return None;
    }
    let has = |c: &crate::style::computed::Computed| {
        c.clip_polygon.is_some()
            || c.clip_inset.is_some()
            || c.clip_bare_box
            || c.clip_shape.as_deref().is_some_and(|s| {
                s.starts_with("shape:") || s.starts_with("pathdef:") || s.starts_with("shapedef:")
            })
    };
    // Каскад сильнее презентационного атрибута; атрибут разбирается тем же
    // `apply_one`, что и CSS-объявление.
    let parsed: crate::style::computed::Computed;
    let (c, view_box) = if has(&e.style) {
        (&e.style, false)
    } else {
        let raw = e
            .attr("clip-path")
            .filter(|v| !v.trim_start().starts_with("url("))?;
        let mut fresh = crate::style::computed::Computed::default();
        fresh.apply_one("clip-path", raw);
        parsed = fresh;
        (&parsed, raw.contains("view-box"))
    };
    if !has(c) {
        return None;
    }
    // Коробки SVG-элемента (css-masking-1): content/padding -> fill-box,
    // border/margin и умолчание -> stroke-box; `view-box` — начало системы
    // `viewBox`, его размер.
    let rb = if view_box {
        let (w, h) = VIEW_BOX.with(|v| v.get());
        (0.0, 0.0, w, h)
    } else {
        shape_box(e, !matches!(c.clip_ref, Some(2) | Some(3)))?
    };
    let body = clip_body(c, rb)?;
    let mut hasher = DefaultHasher::new();
    body.hash(&mut hasher);
    let id = format!("kamin-clip-{:x}", hasher.finish());
    out.push_str(&format!(
        "<clipPath id=\"{id}\" clipPathUnits=\"userSpaceOnUse\">{body}</clipPath>"
    ));
    Some(id)
}

/// Опорная коробка SVG-элемента для `transform-box` (css-transforms-1
/// §transform-box) в его пользовательских точках: `(x, y, ширина, высота)`.
///
/// fill-box — object bounding box (SVG 2 §8.10): `rect`/`image`/`use`/
/// `foreignObject` по атрибутам, `circle`/`ellipse` по центру и радиусам,
/// `g`/`a` — объединение детей (их собственные преобразования не
/// учитываются). stroke-box — она же, раздвинутая на полтолщины обводки,
/// когда обводка есть; у `vector-effect: non-scaling-stroke` толщина задана в
/// точках экрана и сама зависит от преобразования — эталоны
/// `svgbox-stroke-box-003..005` ждут для неё рамку заливки.
pub(super) fn reference_box(e: &Element, stroke: bool) -> Option<(f32, f32, f32, f32)> {
    let num = |name: &str| {
        e.attrs
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| v.trim().trim_end_matches("px").parse::<f32>().ok())
    };
    let fill = match e.tag.as_str() {
        "rect" | "image" | "use" | "foreignObject" => (
            num("x").unwrap_or(0.0),
            num("y").unwrap_or(0.0),
            num("width")?,
            num("height")?,
        ),
        "circle" => {
            let r = num("r")?;
            (
                num("cx").unwrap_or(0.0) - r,
                num("cy").unwrap_or(0.0) - r,
                2.0 * r,
                2.0 * r,
            )
        }
        "ellipse" => {
            let (rx, ry) = (num("rx")?, num("ry")?);
            (
                num("cx").unwrap_or(0.0) - rx,
                num("cy").unwrap_or(0.0) - ry,
                2.0 * rx,
                2.0 * ry,
            )
        }
        "g" | "a" => {
            let mut acc: Option<(f32, f32, f32, f32)> = None;
            for n in &e.children {
                let Node::Element(c) = n else { continue };
                let Some((x, y, w, h)) = reference_box(c, stroke) else {
                    continue;
                };
                acc = Some(match acc {
                    None => (x, y, w, h),
                    Some((ax, ay, aw, ah)) => {
                        let (x0, y0) = (ax.min(x), ay.min(y));
                        let (x1, y1) = ((ax + aw).max(x + w), (ay + ah).max(y + h));
                        (x0, y0, x1 - x0, y1 - y0)
                    }
                });
            }
            return acc;
        }
        _ => return None,
    };
    let non_scaling = e.style.svg_non_scaling == Some(true)
        || e.attrs
            .iter()
            .any(|(k, v)| k == "vector-effect" && v.trim() == "non-scaling-stroke");
    let paint = e
        .attrs
        .iter()
        .find(|(k, _)| k == "stroke")
        .map(|(_, v)| v.clone())
        .or_else(|| e.style.svg_stroke.clone());
    if !stroke || non_scaling || paint.as_deref().is_none_or(|p| p.trim() == "none") {
        return Some(fill);
    }
    let sw = num("stroke-width")
        .or_else(|| {
            e.style
                .svg_stroke_width
                .as_deref()
                .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
        })
        .unwrap_or(1.0);
    let half = sw * 0.5;
    Some((fill.0 - half, fill.1 - half, fill.2 + sw, fill.3 + sw))
}
