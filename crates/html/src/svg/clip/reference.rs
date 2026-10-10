//! Опорная коробка SVG для CSS clip-path.

use crate::dom::{Element, Node};

/// Сдвиг `transform="translate(x[ ,]y)"` — единственный вид, который переносит
/// рамку ребёнка в систему группы без поворота и масштаба.
pub(crate) fn translate_only(t: &str) -> Option<(f32, f32)> {
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
pub(super) fn stroke_half(e: &Element) -> f32 {
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
pub(crate) fn shape_box(e: &Element, stroke: bool) -> Option<(f32, f32, f32, f32)> {
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
pub(crate) fn reference_box(e: &Element, stroke: bool) -> Option<(f32, f32, f32, f32)> {
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
