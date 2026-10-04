//! `<svg>` внутри документа.
//!
//! Зачем отдельно: инструмент ShowWidget прямо предлагает модели рисовать
//! графики и диаграммы через SVG, поэтому без него виджеты теряют главное.
//!
//! Как: поддерево `<svg>` сериализуется обратно в разметку и растрируется
//! теми же resvg/usvg, которыми пользуется сам GPUI, а результат отдаётся как
//! готовое изображение. Путь через `svg()`-элемент не годится: он превращает
//! рисунок в одноцветную маску (это иконочный путь), а график обязан
//! сохранить цвета.
//!
//! Плата за такой подход — растр: при увеличении масштаба картинка не
//! пересчитывается. Поэтому рисуем с запасом по плотности.

use crate::dom::{Element, Node};
use gpui::{AnyElement, ImageSource, IntoElement, RenderImage, Styled};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

/// Во сколько раз растрировать плотнее логического размера: на дробном
/// системном масштабе (125%, 150%) картинка иначе выглядит мыльной.
const DENSITY: f32 = 2.0;

/// Обратная сериализация поддерева в разметку SVG.
///
/// Дерево у нас уже разобрано, исходного текста нет — а растеризатору нужен
/// именно текст. Пишем только то, что имеет смысл внутри SVG: имя тега,
/// атрибуты и текстовые узлы.
pub fn serialize(e: &Element) -> String {
    let mut out = String::new();
    if subtree_has(e, "fedisplacementmap") {
        let mut copy = e.clone();
        untaint_filters(&mut copy);
        write_element(&copy, &mut out);
    } else {
        write_element(e, &mut out);
    }
    out
}

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): дописывать в `<defs>` недостающие
/// `<filter>` из соседних `<svg>` документа (реестр `render::mask_def`,
/// ключ `filter:<id>`) перед `</svg>` — срез filter-effects 184 -> 184,
/// `svg-filter-primitive-units-user-space` и родня не сдвинулись:
/// одной подстановки определения мало, единицы фильтра считаются от
/// чужого вьюпорта. Разбор: target/scout-masking-filters-2026-09.md, F5.
fn subtree_has(e: &Element, tag: &str) -> bool {
    e.tag.eq_ignore_ascii_case(tag)
        || e.children.iter().any(|n| match n {
            Node::Element(c) => subtree_has(c, tag),
            _ => false,
        })
}

/// filter-effects-1 «Restrictions on filter primitives»: примитив, чей цвет
/// зависит от currentColor (`flood-color`/`lighting-color` в любой обёртке),
/// «загрязнён»; `feDisplacementMap` с загрязнённой картой (in2) обязан
/// работать сквозным проходом. resvg о загрязнении не знает — такой примитив
/// заменяется на `feOffset dx=0 dy=0` до сериализации (WPT tainting-*).
fn untaint_filters(e: &mut Element) {
    if e.tag == "filter" {
        let mut tainted: std::collections::HashSet<String> = Default::default();
        let mut prev = false;
        for child in e.children.iter_mut() {
            let Node::Element(p) = child else { continue };
            if !p.tag.starts_with("fe") {
                continue;
            }
            let val =
                |p: &Element, k: &str| p.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
            let dirty_ref = |name: &Option<String>, prev: bool| match name.as_deref() {
                None => prev,
                Some(n) => tainted.contains(n),
            };
            // Собственное загрязнение: цвет примитива от currentColor —
            // в атрибуте или в style, включая обёртки color-mix()/color(from).
            let self_dirty = p.attrs.iter().any(|(k, v)| {
                matches!(k.as_str(), "flood-color" | "lighting-color" | "style")
                    && v.to_ascii_lowercase().contains("currentcolor")
            });
            let in1 = val(p, "in");
            let in2 = val(p, "in2");
            let mut dirty = self_dirty || dirty_ref(&in1, prev);
            if p.tag == "feMerge" {
                for n in &p.children {
                    if let Node::Element(m) = n
                        && m.tag == "feMergeNode"
                        && dirty_ref(&val(m, "in"), prev)
                    {
                        dirty = true;
                    }
                }
            }
            if p.tag == "feDisplacementMap" && dirty_ref(&in2, prev) {
                // Сквозной проход: сохранить in/result, снять карту.
                p.tag = "feOffset".to_string();
                p.attrs.retain(|(k, _)| {
                    !matches!(
                        k.as_str(),
                        "in2" | "scale" | "xChannelSelector" | "yChannelSelector" | "dx" | "dy"
                    )
                });
                p.attrs.push(("dx".to_string(), "0".to_string()));
                p.attrs.push(("dy".to_string(), "0".to_string()));
                // Выход сквозного прохода загрязнён лишь настолько,
                // насколько его основной вход.
                dirty = dirty_ref(&in1, prev);
            } else if p.tag == "feDisplacementMap" {
                dirty = dirty || dirty_ref(&in2, prev);
            }
            if dirty && let Some(r) = val(p, "result") {
                tainted.insert(r);
            }
            prev = dirty;
        }
        return;
    }
    for child in e.children.iter_mut() {
        if let Node::Element(c) = child {
            untaint_filters(c);
        }
    }
}

thread_local! {
    /// Размер опорной коробки `view-box` ближайшего вьюпорта (css-masking-1
    /// §5.1 `view-box`): начало — в начале системы координат `viewBox`,
    /// размер — его ширина и высота; без `viewBox` — размер самого `<svg>`.
    static VIEW_BOX: std::cell::Cell<(f32, f32)> =
        const { std::cell::Cell::new((300.0, 150.0)) };
    /// Пишутся дети `<clipPath>`: свой `<clipPath>` рядом с ними синтезировать
    /// нельзя — модель содержимого `<clipPath>` только фигуры, текст и `use`.
    static IN_CLIP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Сдвиг `transform="translate(x[ ,]y)"` — единственный вид, который переносит
/// рамку ребёнка в систему группы без поворота и масштаба.
fn translate_only(t: &str) -> Option<(f32, f32)> {
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
fn shape_box(e: &Element, stroke: bool) -> Option<(f32, f32, f32, f32)> {
    let num = |k: &str| {
        e.attr(k)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
    };
    let css = |l: Option<crate::value::Len>| match l {
        Some(crate::value::Len::Px(v)) => Some(v),
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
    c: &crate::computed::Computed,
    (bx, by, bw, bh): (f32, f32, f32, f32),
) -> Option<String> {
    use crate::value::Len;
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
        let rule = if c.clip_polygon_evenodd { "evenodd" } else { "nonzero" };
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
        let (cx, cy, rx, ry) = crate::background::shape_params(func, bw, bh, 1.0)?;
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
        (rule, crate::background::shape_to_path(body, bw, bh)?)
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
fn synth_clip(e: &Element, out: &mut String) -> Option<String> {
    if e.tag.eq_ignore_ascii_case("svg") || IN_CLIP.with(|c| c.get()) {
        return None;
    }
    let has = |c: &crate::computed::Computed| {
        c.clip_polygon.is_some()
            || c.clip_inset.is_some()
            || c.clip_bare_box
            || c.clip_shape.as_deref().is_some_and(|s| {
                s.starts_with("shape:") || s.starts_with("pathdef:") || s.starts_with("shapedef:")
            })
    };
    // Каскад сильнее презентационного атрибута; атрибут разбирается тем же
    // `apply_one`, что и CSS-объявление.
    let parsed: crate::computed::Computed;
    let (c, view_box) = if has(&e.style) {
        (&e.style, false)
    } else {
        let raw = e
            .attr("clip-path")
            .filter(|v| !v.trim_start().starts_with("url("))?;
        let mut fresh = crate::computed::Computed::default();
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

pub(crate) fn write_element(e: &Element, out: &mut String) {
    // Синтезированный `<clipPath>` пишется ПЕРЕД элементом (см. `synth_clip`).
    let clip_id = synth_clip(e, out);
    out.push('<');
    out.push_str(&e.tag);
    // `transform-origin` растеризатор не знает — точка отсчёта
    // вкатывается в сам transform парой translate. Одно значение — x,
    // второй осью служит середина fill-box (css-transforms §4);
    // доли — от fill-box (атрибуты width/height фигуры).
    let attr_of = |name: &str| {
        e.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    let num_attr = |name: &str| attr_of(name).and_then(|v| v.trim().parse::<f32>().ok());
    let (fx, fy, fw, fh) = (
        num_attr("x").unwrap_or(0.0),
        num_attr("y").unwrap_or(0.0),
        num_attr("width").unwrap_or(0.0),
        num_attr("height").unwrap_or(0.0),
    );
    // Опорная коробка (css-transforms-1 §transform-box) — только при ЯВНОЙ
    // `fill-box`/`stroke-box` (и их CSS-двойниках `content-box`/`border-box`).
    // Тогда и доли, и ДЛИНЫ `transform-origin` отсчитываются от её левого
    // верха, а проценты `translate()` — от её размера (`fill-box-001`
    // target4: `75px 75px` — это (100,100), а не (75,75)). Без явного
    // значения путь прежний: доли от рамки по атрибутам, длины как есть.
    let ref_box = match e.style.transform_box {
        Some(k @ (1 | 2)) => Some(reference_box(e, k == 2).unwrap_or((fx, fy, fw, fh))),
        _ => None,
    };
    let (bx, by, bw, bh) = ref_box.unwrap_or((fx, fy, fw, fh));
    let from_box = |v: f32, o: f32| if ref_box.is_some() { o + v } else { v };
    // CSS `transform-origin` сильнее презентационного атрибута
    // (css-transforms-1 §specificity): доли — от той же опорной коробки, что
    // и у атрибута, точки — от её начала при явной коробке, иначе как есть
    // (`transform-box/fill-box-*`, `svg-origin-relative-length-*`).
    let style_origin = match (e.style.transform_origin, e.style.transform_origin_px) {
        (_, (Some(px), Some(py))) => Some((from_box(px, bx), from_box(py, by))),
        (Some((kx, ky)), (px, py)) => Some((
            px.map_or(bx + bw * kx, |v| from_box(v, bx)),
            py.map_or(by + bh * ky, |v| from_box(v, by)),
        )),
        _ => None,
    };
    let origin = style_origin.or_else(|| attr_of("transform-origin").and_then(|raw| {
        let side = |t: &str, base: f32, off: f32| -> Option<f32> {
            let t = t.trim();
            Some(match t {
                "left" | "top" => off,
                "center" => off + base * 0.5,
                "right" | "bottom" => off + base,
                _ if t.ends_with('%') => {
                    off + t.trim_end_matches('%').parse::<f32>().ok()? / 100.0 * base
                }
                _ => {
                    // Абсолютные единицы (css-values §6.2): 1in = 96px.
                    let (num, k) = if let Some(n) = t.strip_suffix("px") {
                        (n, 1.0)
                    } else if let Some(n) = t.strip_suffix("cm") {
                        (n, 96.0 / 2.54)
                    } else if let Some(n) = t.strip_suffix("mm") {
                        (n, 96.0 / 25.4)
                    } else if let Some(n) = t.strip_suffix("in") {
                        (n, 96.0)
                    } else if let Some(n) = t.strip_suffix("pt") {
                        (n, 96.0 / 72.0)
                    } else if let Some(n) = t.strip_suffix("pc") {
                        (n, 16.0)
                    } else if let Some(n) = t.strip_suffix('q').or_else(|| t.strip_suffix('Q')) {
                        (n, 96.0 / 101.6)
                    } else {
                        (t, 1.0)
                    };
                    // Длина от рамки фигуры — только при
                    // `transform-box: fill-box`; по умолчанию (`view-box`)
                    // отсчёт от вьюпорта, то есть без сдвига на `off`
                    // (`svg-origin-length-*` зелены именно так).
                    let v = num.trim().parse::<f32>().ok()? * k;
                    return Some(if e.style.transform_box_fill == Some(true) {
                        off + v
                    } else {
                        v
                    });
                }
            })
        };
        let toks: Vec<&str> = raw.split_whitespace().collect();
        // Осевые ключевые слова (css-transforms §4): одиночный `top`/`bottom`
        // — это ось Y с центром по X; пара слов может идти в любом порядке,
        // но `top 100%` невалидна — тогда точка отсчёта остаётся `0 0`
        // (None = без origin-обёртки).
        let vert_only = |t: &str| matches!(t.trim(), "top" | "bottom");
        let horiz_only = |t: &str| matches!(t.trim(), "left" | "right");
        let keyword = |t: &str| matches!(t.trim(), "top" | "bottom" | "left" | "right" | "center");
        let (ox, oy) = match toks.as_slice() {
            [a] if vert_only(a) => (fx + fw * 0.5, side(a, fh, fy)?),
            [a] => (side(a, fw, fx)?, fy + fh * 0.5),
            // Пара слов ОДНОЙ оси невалидна (css-transforms-1 §transform-origin):
            // `top bottom`, `left right` — объявление отбрасывается целиком.
            [a, b]
                if (vert_only(a) && vert_only(b)) || (horiz_only(a) && horiz_only(b)) =>
            {
                return None;
            }
            [a, b] if vert_only(a) || horiz_only(b) => {
                // Обратный порядок допустим только у ПАРЫ ключевых слов.
                if keyword(a) && keyword(b) {
                    (side(b, fw, fx)?, side(a, fh, fy)?)
                } else {
                    return None;
                }
            }
            [a, b] => (side(a, fw, fx)?, side(b, fh, fy)?),
            _ => return None,
        };
        Some((ox, oy))
    }));
    // Начальный `transform-origin` SVG-элемента — `0 0` (UA-лист:
    // `*:not(svg), *:not(foreignObject) > svg { transform-origin: 0 0 }`), и
    // отсчитывается он от опорной коробки: при явной `fill-box`/`stroke-box`
    // это её левый верх, а не начало координат (`fill-box-001` target1:
    // `rotate(90deg)` вокруг (0,0) уводил фигуру за кадр).
    let origin = origin.or(ref_box.map(|(x, y, _, _)| (x, y)));
    // Стилевой transform на SVG-ребёнке СИЛЬНЕЕ презентационного атрибута
    // (css-transforms §specificity) — сериализуется атрибутом для
    // растеризатора.
    // Матрица функций В ПОРЯДКЕ ЗАПИСИ (см. `computed::Transform::lin`);
    // проценты сдвига — от опорной коробки фигуры (css-transforms-1
    // §transform-box: доля — от reference box; здесь fill-box по атрибутам).
    let style_t = e.style.transform.as_ref().map(|t| {
        let unit = t.lin == [[1.0, 0.0], [0.0, 1.0]]
            && t.tr == [[0.0; 3]; 2];
        if unit {
            return String::new();
        }
        let tx = t.tr[0][0] + bw * t.tr[0][1] + bh * t.tr[0][2];
        let ty = t.tr[1][0] + bw * t.tr[1][1] + bh * t.tr[1][2];
        format!(
            "matrix({} {} {} {} {} {})",
            t.lin[0][0], t.lin[1][0], t.lin[0][1], t.lin[1][1], tx, ty
        )
    });
    // Отдельное свойство `translate` (css-transforms-2 §individual-transforms)
    // действует и на SVG-фигуре — она transformable element
    // (css-transforms-1 §transformable-element), — но до растеризатора не
    // доезжало: матрица выше собирается из `lin`/`tr` свойства `transform`, а
    // `translate:` живёт отдельным полем `Computed::translate`, и его
    // единственный потребитель `apply.rs` двигает CSS-КОРОБКУ, внутрь `<svg>`
    // не заходя. Оттого `<rect style="translate: 100px 100px">` стоял на
    // месте: наш зелёный — (10,67)-(134,191) точек устройства, эталонный —
    // (135,192)-(259,316), ровно 100 css-точек по обеим осям, и накрываемый
    // красный оставался виден (`translate/translate-in-svg`).
    //
    // Порядок сборки — css-transforms-2 §ctm: «translate, then rotate, then
    // scale, then transform», то есть сдвиг стоит СЛЕВА и от матрицы свойства
    // `transform`, и от презентационного атрибута `transform=` (тот
    // отображается в то же свойство — css-transforms-1 §svg-transform).
    // Доли — от опорной коробки (§transform-box; здесь fill-box по атрибутам
    // фигуры, как и у процентов сдвига выше).
    //
    // Корневой `<svg>` исключён: он обычная CSS-коробка, и `translate:` ему
    // уже сдвигает `apply.rs` — иначе сдвиг лёг бы дважды.
    let ind_t = e.style.translate.filter(|_| e.tag != "svg").and_then(|(x, y)| {
        let axis = |l: crate::value::Len, base: f32| match l {
            crate::value::Len::Px(v) => v,
            crate::value::Len::Pct(k) => k * base,
            _ => 0.0,
        };
        let (dx, dy) = (axis(x, bw), axis(y, bh));
        (dx != 0.0 || dy != 0.0).then_some((dx, dy))
    });
    // Невалидный список преобразований В АТРИБУТЕ (`rotate(90,)`: запятая без
    // аргумента — грамматика `transform-list`, SVG 1.1 §7.6). Атрибут —
    // презентационная форма свойства `transform` (css-transforms-1
    // §svg-transform), и ошибка разбора отбрасывает его целиком, как
    // невалидное объявление. usvg висячую запятую прощает и поворачивал
    // фигуру (`svg-rotate-3args-invalid-002`, `svg-external-styles-014`).
    let attr_bad = attr_of("transform").is_some_and(|t| {
        let s: String = t.chars().filter(|c| !c.is_whitespace()).collect();
        s.contains(",)") || s.contains("(,") || s.contains(",,")
    });
    let attr_t = attr_of("transform")
        .filter(|_| !attr_bad)
        .map(str::to_string);
    let base_t = style_t.filter(|t| !t.is_empty()).or(attr_t);
    let transform = match (ind_t, base_t) {
        (Some((dx, dy)), Some(t)) => Some(format!("translate({dx} {dy}) {t}")),
        (Some((dx, dy)), None) => Some(format!("translate({dx} {dy})")),
        (None, t) => t,
    };
    let combined = match (origin, transform) {
        (Some((ox, oy)), Some(t)) => Some(format!(
            "translate({ox} {oy}) {t} translate({} {})",
            -ox, -oy
        )),
        // Сдвиг от `translate:` — такой же повод перебить презентационный
        // атрибут, как и своё свойство `transform`: без этой ветки собранная
        // строка терялась, а в разметку уходил нетронутый атрибут.
        (None, Some(t)) if e.style.transform.is_some() || ind_t.is_some() => Some(t),
        _ => None,
    };
    for (k, v) in &e.attrs {
        if combined.is_some() && (k == "transform" || k == "transform-origin") {
            continue;
        }
        // Синтезированная обрезка заменяет CSS-запись фигуры: usvg её не
        // разбирает, а оставленная рядом с нашей `url()` спорила бы с ней.
        if clip_id.is_some() && k == "clip-path" {
            continue;
        }
        if k == "transform-origin" {
            continue;
        }
        if k == "transform" && attr_bad {
            continue;
        }
        // `divisor="0"` у feConvolveMatrix: по спеке берётся умолчание (сумма
        // ядра), а usvg на нуле возвращает ошибку и элемент исчезает.
        if k == "divisor" && v.trim().parse::<f32>().ok() == Some(0.0) {
            continue;
        }
        // Объявления трансформа из `style=` уже учтены в `e.style` и
        // уходят нашим `transform="…"`; в usvg `transform` —
        // презентационный атрибут, и объявление из `style` его ПЕРЕБИВАЕТ,
        // а CSS-запись `translate(100px, 0)` парсер SVG не понимает —
        // получалась единичная матрица (`svg-inline-styles-001..013`).
        let v = if k == "style" {
            let kept: Vec<&str> = v
                .split(';')
                .filter(|d| {
                    let name = d.split(':').next().unwrap_or("").trim();
                    // `clip-path` из `style=` уже ушёл синтезированным
                    // `<clipPath>`: в usvg объявление `style` перебивает
                    // презентационный атрибут и погасило бы нашу ссылку.
                    !(matches!(
                        name,
                        "transform"
                            | "transform-origin"
                            | "transform-box"
                            | "translate"
                            | "rotate"
                            | "scale"
                    ) || (clip_id.is_some() && name == "clip-path"))
                })
                .collect();
            std::borrow::Cow::Owned(kept.join(";"))
        } else {
            std::borrow::Cow::Borrowed(v.as_str())
        };
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        escape_attr(&v, out);
        out.push('"');
    }
    if let Some(t) = &combined {
        out.push_str(" transform=\"");
        escape_attr(t, out);
        out.push('"');
    }
    if let Some(id) = &clip_id {
        out.push_str(&format!(" clip-path=\"url(#{id})\""));
    }
    // CSS-геометрия и заливка SVG-фигур (SVG 2): стилевые ширина/высота
    // и `fill` доезжают до растеризатора презентационными атрибутами,
    // если разметка своих не задала.
    let has = |name: &str| e.attrs.iter().any(|(k, _)| k == name);
    if e.tag != "svg" {
        if let Some(crate::value::Len::Px(w)) = e.style.width
            && !has("width")
        {
            out.push_str(&format!(" width=\"{w}\""));
        }
        if let Some(crate::value::Len::Px(h)) = e.style.height
            && !has("height")
        {
            out.push_str(&format!(" height=\"{h}\""));
        }
    }
    if let Some(fill) = &e.style.svg_fill
        && !has("fill")
    {
        out.push_str(" fill=\"");
        escape_attr(fill, out);
        out.push('"');
    }
    // Обводка и геометрия из КАСКАДА: правило `rect.frame { x: -0.5px;
    // stroke: black }` живёт в `<style>` с селектором, в разметке фигуры его
    // нет, а растеризатор видит только разметку. Разметка сильнее: свой
    // атрибут не перебиваем — как у `fill` выше.
    if let Some(stroke) = &e.style.svg_stroke
        && !has("stroke")
    {
        out.push_str(" stroke=\"");
        escape_attr(stroke, out);
        out.push('"');
    }
    if let Some(w) = &e.style.svg_stroke_width
        && !has("stroke-width")
    {
        // `vector-effect: non-scaling-stroke` из КАСКАДА (SVG 2
        // §vector-effect): толщина — в точках экрана. Само свойство в разметку
        // не уходит, поэтому собственный масштаб фигуры снимается с толщины
        // здесь: равномерный множитель стилевой матрицы √|det| (масштаб
        // предков не учитывается). Без этого `svgbox-stroke-box-003/004` при
        // верной геометрии рисовали обводку вдвое тоньше эталонной.
        let k = match (e.style.svg_non_scaling, e.style.transform) {
            (Some(true), Some(t)) if !has("vector-effect") => {
                (t.lin[0][0] * t.lin[1][1] - t.lin[0][1] * t.lin[1][0]).abs().sqrt()
            }
            _ => 1.0,
        };
        match w.trim().trim_end_matches("px").parse::<f32>() {
            Ok(v) if k > 1e-6 && (k - 1.0).abs() > 1e-6 => {
                out.push_str(&format!(" stroke-width=\"{}\"", v / k));
            }
            _ => {
                out.push_str(" stroke-width=\"");
                escape_attr(w, out);
                out.push('"');
            }
        }
    }
    if e.tag != "svg" {
        if let Some(crate::value::Len::Px(x)) = e.style.svg_x
            && !has("x")
        {
            out.push_str(&format!(" x=\"{x}\""));
        }
        if let Some(crate::value::Len::Px(y)) = e.style.svg_y
            && !has("y")
        {
            out.push_str(&format!(" y=\"{y}\""));
        }
    }
    if e.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    // Вложенный `<svg>` — свой вьюпорт для `view-box` детей; дети
    // `<clipPath>` — без синтеза своей обрезки (см. `IN_CLIP`).
    let prev_vb = VIEW_BOX.with(|v| v.get());
    if e.tag.eq_ignore_ascii_case("svg") {
        let vb = view_box_ratio(e).unwrap_or_else(|| size_of(e));
        VIEW_BOX.with(|v| v.set(vb));
    }
    let prev_clip = IN_CLIP.with(|c| c.get());
    if e.tag.eq_ignore_ascii_case("clippath") {
        IN_CLIP.with(|c| c.set(true));
    }
    for child in &e.children {
        match child {
            Node::Text(t) => escape_text(t, out),
            Node::Element(el) => write_element(el, out),
        }
    }
    IN_CLIP.with(|c| c.set(prev_clip));
    VIEW_BOX.with(|v| v.set(prev_vb));
    out.push_str("</");
    out.push_str(&e.tag);
    out.push('>');
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
fn reference_box(e: &Element, stroke: bool) -> Option<(f32, f32, f32, f32)> {
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
        || e
            .attrs
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

fn escape_attr(v: &str, out: &mut String) {
    for ch in v.chars() {
        match ch {
            '"' => out.push_str("&quot;"),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            c => out.push(c),
        }
    }
}

fn escape_text(v: &str, out: &mut String) {
    for ch in v.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
}

/// Размер рисунка в логических пикселях: из атрибутов, иначе из `viewBox`.
pub fn size_of(e: &Element) -> (f32, f32) {
    // Обособление размера: рисунок меряется как пустой, величину задаёт
    // `contain-intrinsic-size` (css-contain-2 §size containment) — ни
    // атрибуты, ни `viewBox` не смотрим.
    // Атрибутные `width`/`height` — природный размер замещаемого, и зум его
    // домножает («It also multiplies the natural size of all replaced
    // elements», css-viewport-1 §zoom; `zoom/svg-stroke-width`). CSS-размеры
    // ниже уже домножены проходом `zoom::resolve`.
    let z = e.style.zoom_eff.unwrap_or(1.0);
    let num = |name: &str| -> Option<f32> {
        e.attr(name)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
            .map(|v| v * z)
    };
    // Стилевые размеры СТАРШЕ атрибутов (CSS поверх разметки).
    let css = |l: Option<crate::value::Len>| match l {
        Some(crate::value::Len::Px(v)) => Some(v),
        _ => None,
    };
    let given_w = css(e.style.width).or_else(|| num("width"));
    let given_h = css(e.style.height).or_else(|| num("height"));
    // Обособление размера снимает у рисунка ПРИРОДНЫЕ стороны и природное
    // соотношение (`viewBox`), но не написанные автором: «All CSS properties
    // of the size containment box are taken into account as they would be
    // when performing layout normally» (css-contain-2 Overview.bs:681-683).
    // Ось без названного размера берёт `contain-intrinsic-*`, иначе ноль.
    // Раньше выбрасывались ОБЕ стороны, и `<svg width="100" viewBox="0 0 50 50">`
    // под `contain: size` выходил нулевой ширины.
    if e.style.contains_width() || e.style.contains_height() {
        let axis = |contained: bool, given: Option<f32>, ci: Option<f32>| {
            if contained {
                given.or(ci).unwrap_or(0.0)
            } else {
                given.unwrap_or(0.0)
            }
        };
        return (
            axis(e.style.contains_width(), given_w, e.style.contain_intrinsic.0),
            axis(e.style.contains_height(), given_h, e.style.contain_intrinsic.1),
        );
    }
    if let (Some(w), Some(h)) = (given_w, given_h) {
        return (w, h);
    }
    // Заявленное `aspect-ratio` СИЛЬНЕЕ соотношения из `viewBox`
    // (css-sizing-4 §4: «the preferred aspect ratio … overrides any natural
    // aspect ratio»): рисунок переформатируется, как и картинка.
    if let Some(r) = e.style.aspect_ratio.filter(|r| r.is_finite() && *r > 0.0) {
        match (given_w, given_h) {
            (Some(w), None) => return (w, w / r),
            (None, Some(h)) => return (h * r, h),
            _ => {}
        }
    }
    // `viewBox` задаёт СООТНОШЕНИЕ сторон, а не собственный размер: заданная
    // сторона тянет за собой вторую (CSS Images 3 §5 default sizing).
    let ratio = view_box_ratio(e);
    match (given_w, given_h, ratio) {
        (Some(w), None, Some((vw, vh))) => (w, w * vh / vw),
        (None, Some(h), Some((vw, vh))) => (h * vw / vh, h),
        // Ни одной стороны: рисунок сам себе размера не даёт. По CSS 2.1
        // §10.3.2 замещаемый без собственных сторон занимает 300x150, а с
        // соотношением — наибольший такой прямоугольник, что в 300x150
        // влезает. Прежде отдавали viewBox как СОБСТВЕННЫЙ размер и 120x120
        // без него — рисунок выходил своего масштаба, а не блочного.
        (None, None, Some((vw, vh))) => {
            let k = (DEFAULT_REPLACED.0 / vw).min(DEFAULT_REPLACED.1 / vh);
            (vw * k, vh * k)
        }
        (Some(w), None, None) => (w, DEFAULT_REPLACED.1),
        (None, Some(h), None) => (DEFAULT_REPLACED.0, h),
        _ => DEFAULT_REPLACED,
    }
}

/// Размер замещаемого элемента, у которого нет собственного (CSS 2.1
/// §10.3.2: «the used value of 'width' becomes 300px … 'height' becomes
/// 150px»).
const DEFAULT_REPLACED: (f32, f32) = (300.0, 150.0);

/// Соотношение сторон из `viewBox`: (ширина, высота) окна просмотра.
fn view_box_ratio(e: &Element) -> Option<(f32, f32)> {
    e.attr("viewBox").and_then(|vb| {
        let p: Vec<f32> = vb
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        (p.len() == 4 && p[2] > 0.0 && p[3] > 0.0).then(|| (p[2], p[3]))
    })
}

/// Рисунок БЕЗ собственного размера, но С соотношением (`<svg viewBox>` без
/// `width`/`height`) занимает stretch-fit ширину содержащего блока, а высоту
/// берёт из соотношения. CSS 2.2 §10.3.2, последний пункт: «if the containing
/// block's width does not itself depend on the replaced element's width, then
/// the used value of 'width' is calculated from the constraint equation used
/// for block-level, non-replaced elements in normal flow»; css-sizing-3 §5.1
/// «stretch-fit size: the size a box would take if its outer size filled the
/// available space»; Blink `ComputeReplacedSizeInternal` (length_utils.cc):
/// без natural size главная сторона — `Length::Stretch()` / `StretchFit()`
/// от available-size, когда тот definite. Резерв 300×150 остаётся только
/// когда ширину взять неоткуда (`cb_width == None`).
///
/// Пределы держат соотношение (CSS 2 §10.4): сперва `max-width`, затем
/// `max-height`; нарушенный предел переносится во вторую ось. До правки
/// `size_of` вписывал рисунок в 300×150 и пределов не читал:
/// `svg-root-as-flex-item-003` рисовал 150×150 вместо 100×100,
/// `flex-aspect-ratio-img-row-015` не видел `max-height: 100px`.
///
/// Возвращает копию с обеими сторонами в точках — их и возьмёт `size_of`.
pub fn stretch_fit(e: &Element, cb_width: Option<f32>) -> Element {
    use crate::value::Len;
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let attr_num = |name: &str| -> bool {
        e.attr(name)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
            .is_some()
    };
    let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
    // Обособление размера и любая заданная сторона (стилем или атрибутом)
    // — прежний путь `size_of`, здесь ничего не меняется.
    if e.style.contains_width()
        || e.style.contains_height()
        || !auto(e.style.width)
        || !auto(e.style.height)
        || attr_num("width")
        || attr_num("height")
    {
        return e.clone();
    }
    let Some((vw, vh)) = view_box_ratio(e) else {
        return e.clone();
    };
    // Заявленное `aspect-ratio` старше соотношения из `viewBox`
    // (css-sizing-4 §4) — та же иерархия, что в `size_of`.
    let ratio = e
        .style
        .aspect_ratio
        .filter(|r| r.is_finite() && *r > 0.0)
        .unwrap_or(vw / vh);
    let side = |l: Option<Len>| px(l).unwrap_or(0.0);
    let b = e.style.borders();
    let pb_x = side(e.style.padding.left) + side(e.style.padding.right) + side(b.left) + side(b.right);
    let pb_y = side(e.style.padding.top) + side(e.style.padding.bottom) + side(b.top) + side(b.bottom);
    // Внешний размер = содержимое + паддинг + рамка + поля (css-sizing-3
    // §5.1 «outer size»); `size_of` отдаёт размер СОДЕРЖИМОГО.
    let outer_x = side(e.style.margin.left) + side(e.style.margin.right) + pb_x;
    let (mut w, mut h) = match cb_width {
        Some(cb) if cb > 0.0 => {
            let w = (cb - outer_x).max(0.0);
            (w, w / ratio)
        }
        _ => {
            let k = (DEFAULT_REPLACED.0 / vw).min(DEFAULT_REPLACED.1 / vh);
            (vw * k, vh * k)
        }
    };
    // При `box-sizing: border-box` предел включает паддинг и рамку —
    // содержимому остаётся остальное (как `sub_w`/`sub_h` в `image_with`).
    let (sub_x, sub_y) = if e.style.border_box == Some(true) { (pb_x, pb_y) } else { (0.0, 0.0) };
    if let Some(m) = px(e.style.max_width).map(|m| (m - sub_x).max(0.0))
        && w > m
    {
        w = m;
        h = w / ratio;
    }
    if let Some(m) = px(e.style.max_height).map(|m| (m - sub_y).max(0.0))
        && h > m
    {
        h = m;
        w = h * ratio;
    }
    let mut copy = e.clone();
    copy.style.width = Some(Len::Px(w));
    copy.style.height = Some(Len::Px(h));
    copy
}

/// Кэш готовых растров: ключ — разметка и размер.
///
/// Без него рисунок растрировался бы на КАЖДЫЙ кадр — элементы в GPUI
/// пересоздаются каждый раз, и вместе с ними пересоздавалась бы картинка.
/// Для графика в чате это десятки миллисекунд на кадр вместо нуля.
type Cache = HashMap<(u64, u32, u32), Option<Arc<RenderImage>>>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

/// Потолок: рисунки в чате мелкие, но их может накопиться много за длинную
/// сессию — держим последние, а не всё подряд.
const CACHE_CAP: usize = 128;

/// Растрировать рисунок. `None`, если разметка не разбирается — тогда
/// вызывающий покажет запасной текст, а не пустое место.
pub fn rasterize(markup: &str, w: f32, h: f32) -> Option<Arc<RenderImage>> {
    let mut hasher = DefaultHasher::new();
    markup.hash(&mut hasher);
    let key = (hasher.finish(), w.round() as u32, h.round() as u32);

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock()
        && let Some(hit) = map.get(&key)
    {
        return hit.clone();
    }
    // Растеризатор берём у самого GPUI: держать второй комплект resvg/usvg
    // ради той же работы — лишний вес бинаря и лишний источник расхождений.
    let image = gpui::svg_markup_to_image(markup, w, h, DENSITY);
    if let Ok(mut map) = cache.lock() {
        if map.len() >= CACHE_CAP {
            map.clear();
        }
        map.insert(key, image.clone());
    }
    image
}

/// Готовый элемент с рисунком либо `None`, если разобрать не удалось.
pub fn element(e: &Element) -> Option<AnyElement> {
    let (w, h) = size_of(e);
    // `overflow: visible` по оси выпускает фигуры за канву (SVG 2 §overflow):
    // растр расширяется до содержимого по свободной оси, а коробка остаётся
    // размером канвы — картинка переполняет её, как в браузере.
    let px_len = |l: Option<crate::value::Len>| match l {
        Some(crate::value::Len::Px(v)) => Some(v),
        _ => None,
    };
    let child_extent = |horiz: bool| -> f32 {
        let mut m: f32 = 0.0;
        for c in &e.children {
            if let Node::Element(el) = c {
                let own = if horiz {
                    px_len(el.style.width).or_else(|| el.attr("width").and_then(|v| v.parse().ok()))
                } else {
                    px_len(el.style.height)
                        .or_else(|| el.attr("height").and_then(|v| v.parse().ok()))
                };
                let at = if horiz {
                    el.attr("x").and_then(|v| v.parse().ok()).unwrap_or(0.0)
                } else {
                    el.attr("y").and_then(|v| v.parse().ok()).unwrap_or(0.0)
                };
                if let Some(v) = own {
                    m = m.max(at + v);
                }
                // Переполнение считается по stroke-box и с учётом чистого
                // сдвига (SVG 2 §overflow: видна вся отрисовка, включая
                // обводку): `<rect stroke-width=20 transform=translate(10 20)>`
                // в `overflow: visible` канве 230×240 рисуется до 240×250
                // (border-shape-shadow-ref). Поворот и масштаб — как прежде.
                if let Some((bx, by, bw, bh)) = shape_box(el, true) {
                    let (mut dx, mut dy) = el
                        .attr("transform")
                        .and_then(translate_only)
                        .unwrap_or((0.0, 0.0));
                    let pure = |t: &crate::computed::Transform| {
                        t.rotate_rad == 0.0
                            && t.skew_rad == (0.0, 0.0)
                            && t.scale == (1.0, 1.0)
                            && t.translate_pct == (0.0, 0.0)
                    };
                    match el.style.transform.as_ref() {
                        Some(t) if pure(t) => {
                            dx += t.translate.0;
                            dy += t.translate.1;
                        }
                        Some(_) => continue,
                        None => {}
                    }
                    if let Some((tx, ty)) = el.style.translate {
                        dx += px_len(Some(tx)).unwrap_or(0.0);
                        dy += px_len(Some(ty)).unwrap_or(0.0);
                    }
                    m = m.max(if horiz { bx + dx + bw } else { by + dy + bh });
                }
            }
        }
        m
    };
    // `contain: paint` перебивает видимое переполнение: край обрезки —
    // коробка плюс `overflow-clip-margin` (css-overflow-3 §overflow-clip).
    let contained = e.style.contain_paint == Some(true);
    let clip_margin = if contained {
        e.style.clip_margin.unwrap_or(0.0)
    } else {
        0.0
    };
    let visible_y = !contained && e.style.overflow_y == Some(crate::computed::Overflow::Visible);
    let visible_x = !contained && e.style.overflow_x == Some(crate::computed::Overflow::Visible);
    let rw = if visible_x {
        w.max(child_extent(true))
    } else {
        (w + clip_margin).min(child_extent(true).max(w))
    };
    let rh = if visible_y {
        h.max(child_extent(false))
    } else {
        (h + clip_margin).min(child_extent(false).max(h))
    };
    // CSS-маски на детях рисунка — слоями (см. `masked_layers`).
    if let Some(layers) = masked_layers(e, w, h, rw, rh) {
        return Some(layers);
    }
    // Переполнение ВЛЕВО и ВВЕРХ (SVG 2 §overflow, `overflow: visible`):
    // stroke-box ребёнка с чистым сдвигом уходит за начало канвы — канва
    // растёт в минус, `viewBox` сдвигается (`serialize_sized`), картинка
    // кладётся с отрицательным краем (border-shape-clips-background-ref:
    // круг r=55 на канве 100×100 выступает на 5 px со всех сторон).
    let neg_extent = |horiz: bool| -> f32 {
        let mut m: f32 = 0.0;
        for c in &e.children {
            let Node::Element(el) = c else { continue };
            let Some((bx, by, _, _)) = shape_box(el, true) else { continue };
            let (mut dx, mut dy) = el
                .attr("transform")
                .and_then(translate_only)
                .unwrap_or((0.0, 0.0));
            match el.style.transform.as_ref() {
                Some(t)
                    if t.rotate_rad == 0.0
                        && t.skew_rad == (0.0, 0.0)
                        && t.scale == (1.0, 1.0)
                        && t.translate_pct == (0.0, 0.0) =>
                {
                    dx += t.translate.0;
                    dy += t.translate.1;
                }
                Some(_) => continue,
                None => {}
            }
            if let Some((tx, ty)) = el.style.translate {
                dx += px_len(Some(tx)).unwrap_or(0.0);
                dy += px_len(Some(ty)).unwrap_or(0.0);
            }
            let at = if horiz { bx + dx } else { by + dy };
            m = m.max(-at);
        }
        m
    };
    let nx = if visible_x { neg_extent(true) } else { 0.0 };
    let ny = if visible_y { neg_extent(false) } else { 0.0 };
    let (cw, ch) = (rw + nx, rh + ny);
    let image = rasterize(&serialize_sized(e, rw, rh, nx, ny), cw, ch)?;
    let mut img = gpui::img(ImageSource::Render(image))
        .w(gpui::px(cw))
        .h(gpui::px(ch));
    // CSS-фон самого <svg> (`svg { background: green }`): канва растра
    // прозрачна, фон красится коробкой картинки (svg-scale-001 и родня).
    if let Some(bg) = e.style.background {
        use gpui::Styled as _;
        img = img.bg(bg.to_hsla());
    }
    if rw > w + 0.5 || rh > h + 0.5 || nx > 0.0 || ny > 0.0 {
        Some({
            use gpui::ParentElement as _;
            use gpui::Styled as _;
            gpui::div()
                .w(gpui::px(w))
                .h(gpui::px(h))
                .flex_shrink_0()
                .child(img.absolute().top(gpui::px(-ny)).left(gpui::px(-nx)))
                .into_any_element()
        })
    } else {
        Some(img.into_any_element())
    }
}

/// Единица рисунка в порядке отрисовки: прямой ребёнок `<svg>` (`inner` =
/// None) либо ребёнок группы `<g>` с чистым сдвигом (`inner` = его номер).
#[derive(Clone, Copy, PartialEq)]
struct Unit {
    top: usize,
    inner: Option<usize>,
}

/// Прямоугольник (x, y, w, h) в CSS-точках коробки `<svg>`.
type Rect = (f32, f32, f32, f32);

/// Маскированная единица: stroke-box (коробка слоя), fill-box, стиль.
type MaskedUnit = (Unit, Rect, Rect, crate::computed::Computed);

/// Теги, которые ничего не рисуют и нужны любому срезу разметки
/// (определения, стили, заголовки).
fn non_rendering(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "defs"
            | "style"
            | "title"
            | "desc"
            | "metadata"
            | "lineargradient"
            | "radialgradient"
            | "pattern"
            | "mask"
            | "clippath"
            | "filter"
            | "symbol"
            | "marker"
    )
}

/// Пользовательские единицы → CSS-точки коробки `<svg>` w×h: масштаб и
/// сдвиг. При `viewBox` — по нему (только равномерный масштаб: иначе
/// `preserveAspectRatio` центрирует, и прямой пересчёт неверен); без него
/// — `zoom` (css-viewport-1 §zoom, см. `serialize_sized`).
fn user_to_css(e: &Element, w: f32, h: f32) -> Option<(f32, f32, f32)> {
    if let Some(vb) = e.attr("viewBox") {
        let p: Vec<f32> = vb
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        if p.len() != 4 || p[2] <= 0.0 || p[3] <= 0.0 {
            return None;
        }
        let (sx, sy) = (w / p[2], h / p[3]);
        if (sx - sy).abs() > 1e-3 {
            return None;
        }
        return Some((sx, -p[0] * sx, -p[1] * sx));
    }
    Some((e.style.zoom_eff.unwrap_or(1.0), 0.0, 0.0))
}

/// Дети `<svg>` с CSS-маской (`mask-image` из каскада): единица и её
/// stroke-box в CSS-точках коробки рисунка. `None` — масок нет либо у
/// какой-то рамка не считается (путь прежний, без маски).
///
/// css-masking-1 §7 применяется и к SVG-элементам, но usvg маску из CSS не
/// знает (`write_element` её не мостит): слой маскированного ребёнка
/// режется отдельным растром и оборачивается `render::grouped` с ЕГО
/// `Computed` — тем же путём, что HTML-коробка (scout-masking A2).
fn masked_units(e: &Element, w: f32, h: f32) -> Option<Vec<MaskedUnit>> {
    let (s, ox, oy) = user_to_css(e, w, h)?;
    let mut out = Vec::new();
    // Рамки ребёнка в CSS-точках: stroke-box (коробка слоя) и fill-box.
    let box_of = |c: &Element, dx: f32, dy: f32| -> Option<(Rect, Rect)> {
        if c.style.transform.is_some() || c.style.translate.is_some() {
            return None;
        }
        let (tx, ty) = match c.attr("transform") {
            Some(t) => translate_only(t)?,
            None => (0.0, 0.0),
        };
        let place = |(bx, by, bw, bh): (f32, f32, f32, f32)| -> Rect {
            (
                (bx + tx + dx) * s + ox,
                (by + ty + dy) * s + oy,
                bw * s,
                bh * s,
            )
        };
        Some((place(shape_box(c, true)?), place(shape_box(c, false)?)))
    };
    for (i, n) in e.children.iter().enumerate() {
        let Node::Element(c) = n else { continue };
        if c.style.mask_image.is_some() {
            let (sb, fb) = box_of(c, 0.0, 0.0)?;
            out.push((Unit { top: i, inner: None }, sb, fb, c.style.clone()));
            continue;
        }
        if c.tag.eq_ignore_ascii_case("g") && c.style.transform.is_none() && c.style.translate.is_none() {
            let (gx, gy) = match c.attr("transform") {
                Some(t) => match translate_only(t) {
                    Some(v) => v,
                    None => continue,
                },
                None => (0.0, 0.0),
            };
            for (j, m) in c.children.iter().enumerate() {
                let Node::Element(k) = m else { continue };
                if k.style.mask_image.is_some() {
                    let (sb, fb) = box_of(k, gx, gy)?;
                    out.push((Unit { top: i, inner: Some(j) }, sb, fb, k.style.clone()));
                }
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

/// Копия `<svg>` только с указанными единицами (и всем нерисующим):
/// группа `<g>` остаётся с подмножеством детей.
fn filtered(e: &Element, keep: &dyn Fn(Unit) -> bool) -> Element {
    let mut copy = e.clone();
    let mut kids = Vec::new();
    for (i, n) in e.children.iter().enumerate() {
        let Node::Element(c) = n else { continue };
        if non_rendering(&c.tag) {
            kids.push(n.clone());
            continue;
        }
        if keep(Unit { top: i, inner: None }) {
            kids.push(n.clone());
            continue;
        }
        if c.tag.eq_ignore_ascii_case("g") {
            let mut g = c.clone();
            g.children = c
                .children
                .iter()
                .enumerate()
                .filter(|(j, m)| match m {
                    Node::Element(k) => non_rendering(&k.tag) || keep(Unit { top: i, inner: Some(*j) }),
                    Node::Text(_) => false,
                })
                .map(|(_, m)| m.clone())
                .collect();
            if g.children.iter().any(|m| matches!(m, Node::Element(k) if !non_rendering(&k.tag))) {
                kids.push(Node::Element(g));
            }
        }
    }
    copy.children = kids;
    copy
}

/// Рисунок с CSS-масками на детях: срезы разметки по порядку отрисовки —
/// пробеги немаскированных единиц полноразмерными растрами, каждая
/// маскированная — своим растром по её рамке в обёртке `render::grouped`
/// (маска, `mask-size/-repeat/-position/-mode` — из её стиля). Порядок
/// наложения сохраняется: слои кладутся абсолютно друг над другом.
fn masked_layers(e: &Element, w: f32, h: f32, rw: f32, rh: f32) -> Option<AnyElement> {
    use gpui::{ParentElement as _, Styled as _};
    let units = masked_units(e, w, h)?;
    // Пользовательская единица в CSS-точках (та же, что у `masked_units`).
    let (s, _, _) = user_to_css(e, w, h)?;
    let masked: Vec<Unit> = units.iter().map(|(u, _, _, _)| *u).collect();
    // Края коробки `kind` от коробки слоя внутрь (t/r/b/l): fill-box,
    // stroke-box (= border-/margin-box у SVG, css-masking-1 §7.10: «for
    // border-box and margin-box is stroke-box», для content-/padding-box —
    // fill-box), view-box — вьюпорт рисунка (0, 0, w, h).
    let edges = |kind: Option<u8>, layer: Rect, fill: Rect| -> [f32; 4] {
        let (lx, ly, lw, lh) = layer;
        let (bx, by, bw, bh) = match kind {
            Some(2) | Some(3) | Some(4) => fill,
            Some(6) => (0.0, 0.0, w, h),
            _ => layer,
        };
        [by - ly, (lx + lw) - (bx + bw), (ly + lh) - (by + bh), bx - lx]
    };
    // Последовательность единиц в порядке отрисовки.
    let mut seq: Vec<Unit> = Vec::new();
    for (i, n) in e.children.iter().enumerate() {
        let Node::Element(c) = n else { continue };
        if non_rendering(&c.tag) {
            continue;
        }
        let split = c.tag.eq_ignore_ascii_case("g") && masked.iter().any(|u| u.top == i && u.inner.is_some());
        if split {
            for (j, m) in c.children.iter().enumerate() {
                if matches!(m, Node::Element(k) if !non_rendering(&k.tag)) {
                    seq.push(Unit { top: i, inner: Some(j) });
                }
            }
        } else {
            seq.push(Unit { top: i, inner: None });
        }
    }
    let mut root = gpui::div().w(gpui::px(w)).h(gpui::px(h)).flex_shrink_0();
    if let Some(bg) = e.style.background {
        root = root.bg(bg.to_hsla());
    }
    let mut run: Vec<Unit> = Vec::new();
    let flush = |run: &mut Vec<Unit>, root: gpui::Div| -> gpui::Div {
        if run.is_empty() {
            return root;
        }
        let keep = run.clone();
        run.clear();
        let part = filtered(e, &|u| keep.contains(&u));
        let Some(img) = rasterize(&serialize_sized(&part, rw, rh, 0.0, 0.0), rw, rh) else {
            return root;
        };
        root.child(
            gpui::img(ImageSource::Render(img))
                .w(gpui::px(rw))
                .h(gpui::px(rh))
                .absolute()
                .top_0()
                .left_0(),
        )
    };
    for u in seq {
        let Some((_, layer_box, fill_box, style)) = units.iter().find(|(m, _, _, _)| *m == u) else {
            run.push(u);
            continue;
        };
        let (bx, by, bw, bh) = layer_box;
        root = flush(&mut run, root);
        if *bw <= 0.0 || *bh <= 0.0 {
            continue;
        }
        let mut style = style.clone();
        style.mask_box_override = Some((
            edges(style.mask_origin, *layer_box, *fill_box),
            style
                .mask_clip
                .filter(|k| *k != 255)
                .map(|k| edges(Some(k), *layer_box, *fill_box)),
        ));
        // Маска живёт в пользовательских единицах ребёнка (css-masking-1
        // §7: длины и `auto`-размер — в системе координат элемента): при
        // `viewBox` 0 0 100 100 на 200×200 рисунок-маска 50×50 кроет
        // 100×100 CSS-точек (mask-origin-3, mask-clip-2). Точечные `mask-size`
        // и `mask-position` переводятся здесь, интринзик — в `Grouped`.
        if (s - 1.0).abs() > 1e-3 {
            let scale_len = |l: crate::value::Len| match l {
                crate::value::Len::Px(v) => crate::value::Len::Px(v * s),
                other => other,
            };
            style.mask_user_scale = s;
            style.mask_size = style.mask_size.map(|(x, y)| (scale_len(x), scale_len(y)));
            style.mask_pos = style.mask_pos.map(|(x, y)| (scale_len(x), scale_len(y)));
            if let Some(list) = style.mask_pos_list.as_mut() {
                for (x, y, _, _) in list.iter_mut() {
                    *x = scale_len(*x);
                    *y = scale_len(*y);
                }
            }
        }
        let style = &style;
        // Срез по рамке: внешняя канва размером с рамку, `viewBox` — её
        // окно в CSS-точках коробки рисунка (вложенный `<svg>` — вьюпорт
        // с прежними единицами).
        let part = filtered(e, &|m| m == u);
        let markup = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{bw}" height="{bh}" viewBox="{bx} {by} {bw} {bh}">{}</svg>"#,
            serialize_sized(&part, rw, rh, 0.0, 0.0)
        );
        let Some(img) = rasterize(&markup, *bw, *bh) else {
            continue;
        };
        let layer = gpui::img(ImageSource::Render(img))
            .w(gpui::px(*bw))
            .h(gpui::px(*bh))
            .into_any_element();
        let layer = crate::render::grouped(layer, style);
        root = root.child(
            gpui::div()
                .absolute()
                .top(gpui::px(*by))
                .left(gpui::px(*bx))
                .w(gpui::px(*bw))
                .h(gpui::px(*bh))
                .child(layer),
        );
    }
    root = flush(&mut run, root);
    Some(root.into_any_element())
}

/// Разметка с корневой канвой ЗАДАННОГО размера: собственные width/height
/// корня заменяются, иначе растеризатор вписал бы рисунок по ним.
fn serialize_sized(e: &Element, w: f32, h: f32, nx: f32, ny: f32) -> String {
    let cleaned;
    let e = if subtree_has(e, "fedisplacementmap") {
        let mut copy = e.clone();
        untaint_filters(&mut copy);
        cleaned = copy;
        &cleaned
    } else {
        e
    };
    let mut out = String::new();
    // Канва с запасом влево/вверх (`nx`, `ny` — переполнение в минус).
    let (cw, ch) = (w + nx, h + ny);
    out.push_str(&format!("<svg width=\"{cw}\" height=\"{ch}\""));
    // Пространство имён обязательно: разметка идёт растеризатору отдельным
    // документом. В XHTML рисунок часто пишут с ПРЕФИКСОМ (`<svg:svg
    // xmlns:svg=…>`), собственного `xmlns` у корня нет, а имена мы храним
    // без префикса — без этой строки usvg не разбирал документ вовсе, и
    // рисунок пропадал целиком (вся семья `*-replaced-*` в CSS2).
    if !e.attrs.iter().any(|(k, _)| k == "xmlns") {
        out.push_str(" xmlns=\"http://www.w3.org/2000/svg\"");
    }
    // Под зумом без `viewBox` пользовательские единицы обязаны вырасти вместе
    // с канвой (длины внутри рисунка — тоже used-значения, css-viewport-1
    // §zoom): `viewBox` в незумленных единицах растягивает содержимое на
    // зумленную канву (`zoom/svg-path`, `zoom/svg`). С `viewBox` масштаб уже
    // задаёт он сам.
    let z = e.style.zoom_eff.unwrap_or(1.0);
    let shifted = nx > 0.0 || ny > 0.0;
    if ((z - 1.0).abs() > f32::EPSILON || shifted)
        && !e.attrs.iter().any(|(k, _)| k.eq_ignore_ascii_case("viewbox"))
    {
        out.push_str(&format!(
            " viewBox=\"{} {} {} {}\"",
            -nx / z,
            -ny / z,
            cw / z,
            ch / z
        ));
    }
    // Канва шире коробки (`overflow: visible`, см. `element`): при `viewBox`
    // пользовательские единицы обязаны остаться прежними, иначе рисунок
    // растянулся бы на выросшую канву (`border-shape-shadow-ref`: рамка
    // 220 → 230). `viewBox` расширяется в той же пропорции, что и канва, а
    // при запасе влево/вверх — сдвигается на него.
    let (w0, h0) = size_of(e);
    let grown = w > w0 + 0.5 || h > h0 + 0.5 || shifted;
    for (k, v) in &e.attrs {
        if k == "width" || k == "height" {
            continue;
        }
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        if grown && k.eq_ignore_ascii_case("viewbox") {
            let p: Vec<f32> = v
                .split([' ', ','])
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse().ok())
                .collect();
            if p.len() == 4 && w0 > 0.0 && h0 > 0.0 {
                let (ux, uy) = (p[2] / w0, p[3] / h0);
                out.push_str(&format!(
                    "{} {} {} {}",
                    p[0] - nx * ux,
                    p[1] - ny * uy,
                    cw * ux,
                    ch * uy
                ));
                out.push('"');
                continue;
            }
        }
        escape_attr(v, &mut out);
        out.push('"');
    }
    out.push('>');
    // Опорная коробка `view-box` для CSS-обрезки детей (`synth_clip`): корень
    // пишется здесь, а не в `write_element`.
    VIEW_BOX.with(|v| v.set(view_box_ratio(e).unwrap_or_else(|| size_of(e))));
    IN_CLIP.with(|c| c.set(false));
    for child in &e.children {
        match child {
            Node::Text(t) => escape_text(t, &mut out),
            Node::Element(el) => write_element(el, &mut out),
        }
    }
    out.push_str("</svg>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::parse;

    fn find_svg(nodes: &[Node]) -> Option<&Element> {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.tag == "svg" {
                    return Some(e);
                }
                if let Some(found) = find_svg(&e.children) {
                    return Some(found);
                }
            }
        }
        None
    }

    #[test]
    fn subtree_is_serialised_back_to_markup() {
        let nodes = parse(
            r##"<svg viewBox="0 0 10 10"><rect width="10" height="10" fill="#f00"/></svg>"##,
            "",
        );
        let svg = find_svg(&nodes).unwrap();
        let out = serialize(svg);
        assert!(out.starts_with("<svg"), "корень на месте: {out}");
        assert!(out.contains("<rect"), "потомок на месте: {out}");
        assert!(
            out.contains(r##"fill="#f00""##),
            "атрибуты сохранены: {out}"
        );
    }

    #[test]
    fn size_comes_from_attributes_then_viewbox() {
        let nodes = parse(r#"<svg width="40" height="20"></svg>"#, "");
        assert_eq!(size_of(find_svg(&nodes).unwrap()), (40.0, 20.0));

        // `viewBox` даёт СООТНОШЕНИЕ, а не собственный размер: без своих
        // сторон замещаемый занимает наибольший прямоугольник этого
        // соотношения, влезающий в 300x150 (CSS 2.1 §10.3.2).
        let nodes = parse(r#"<svg viewBox="0 0 64 32"></svg>"#, "");
        assert_eq!(size_of(find_svg(&nodes).unwrap()), (300.0, 150.0));

        // Заданная сторона тянет за собой вторую по тому же соотношению.
        let nodes = parse(r#"<svg width="80" viewBox="0 0 64 32"></svg>"#, "");
        assert_eq!(size_of(find_svg(&nodes).unwrap()), (80.0, 40.0));
    }

    #[test]
    fn colours_survive_rasterisation() {
        // Ключевая проверка: путь через иконочный элемент отдал бы одноцветную
        // маску, а график обязан сохранить цвета.
        let markup = r##"<svg viewBox="0 0 2 1" xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="1" height="1" fill="#ff0000"/>
            <rect x="1" y="0" width="1" height="1" fill="#0000ff"/>
        </svg>"##;
        let img = rasterize(markup, 2.0, 1.0).expect("растрируется");
        let size = img.size(0);
        let bytes = img.as_bytes(0).expect("пиксели доступны");
        let w = u32::from(size.width) as usize;
        let px_at = |x: usize| {
            let i = x * 4;
            [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]
        };
        let left = px_at(0);
        let right = px_at(w - 1);
        // Порядок каналов BGRA: у красного велик третий байт, у синего первый.
        assert!(left[2] > 200 && left[0] < 60, "слева красный: {left:?}");
        assert!(right[0] > 200 && right[2] < 60, "справа синий: {right:?}");
    }

    #[test]
    fn broken_markup_yields_nothing_rather_than_a_blank() {
        assert!(rasterize("<svg", 10.0, 10.0).is_none());
    }
}
