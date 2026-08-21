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
            let val = |p: &Element, k: &str| {
                p.attrs
                    .iter()
                    .find(|(a, _)| a == k)
                    .map(|(_, v)| v.clone())
            };
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

pub(crate) fn write_element(e: &Element, out: &mut String) {
    out.push('<');
    out.push_str(&e.tag);
    // `transform-origin` растеризатор не знает — точка отсчёта
    // вкатывается в сам transform парой translate. Одно значение — x,
    // второй осью служит середина fill-box (css-transforms §4);
    // доли — от fill-box (атрибуты width/height фигуры).
    let attr_of = |name: &str| e.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str());
    let num_attr = |name: &str| attr_of(name).and_then(|v| v.trim().parse::<f32>().ok());
    let origin = attr_of("transform-origin").and_then(|raw| {
        let (fx, fy, fw, fh) = (
            num_attr("x").unwrap_or(0.0),
            num_attr("y").unwrap_or(0.0),
            num_attr("width").unwrap_or(0.0),
            num_attr("height").unwrap_or(0.0),
        );
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
                    num.trim().parse::<f32>().ok()? * k
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
        let keyword =
            |t: &str| matches!(t.trim(), "top" | "bottom" | "left" | "right" | "center");
        let (ox, oy) = match toks.as_slice() {
            [a] if vert_only(a) => (fx + fw * 0.5, side(a, fh, fy)?),
            [a] => (side(a, fw, fx)?, fy + fh * 0.5),
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
    });
    // Стилевой transform на SVG-ребёнке СИЛЬНЕЕ презентационного атрибута
    // (css-transforms §specificity) — сериализуется атрибутом для
    // растеризатора.
    let style_t = e.style.transform.as_ref().map(|t| {
        let mut out = String::new();
        if t.translate != (0.0, 0.0) {
            out.push_str(&format!("translate({} {}) ", t.translate.0, t.translate.1));
        }
        if t.rotate_rad != 0.0 {
            out.push_str(&format!("rotate({}) ", t.rotate_rad.to_degrees()));
        }
        if t.skew_rad.0 != 0.0 {
            out.push_str(&format!("skewX({}) ", t.skew_rad.0.to_degrees()));
        }
        if t.skew_rad.1 != 0.0 {
            out.push_str(&format!("skewY({}) ", t.skew_rad.1.to_degrees()));
        }
        if t.scale != (1.0, 1.0) {
            out.push_str(&format!("scale({} {}) ", t.scale.0, t.scale.1));
        }
        out.trim_end().to_string()
    });
    let attr_t = attr_of("transform").map(str::to_string);
    let transform = style_t.filter(|t| !t.is_empty()).or(attr_t);
    let combined = match (origin, transform) {
        (Some((ox, oy)), Some(t)) => Some(format!(
            "translate({ox} {oy}) {t} translate({} {})",
            -ox, -oy
        )),
        (None, Some(t)) if e.style.transform.is_some() => Some(t),
        _ => None,
    };
    for (k, v) in &e.attrs {
        if combined.is_some() && (k == "transform" || k == "transform-origin") {
            continue;
        }
        if k == "transform-origin" {
            continue;
        }
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        escape_attr(v, out);
        out.push('"');
    }
    if let Some(t) = &combined {
        out.push_str(" transform=\"");
        escape_attr(t, out);
        out.push('"');
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
    if e.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for child in &e.children {
        match child {
            Node::Text(t) => escape_text(t, out),
            Node::Element(el) => write_element(el, out),
        }
    }
    out.push_str("</");
    out.push_str(&e.tag);
    out.push('>');
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
    let num = |name: &str| -> Option<f32> {
        e.attr(name)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
    };
    // Стилевые размеры СТАРШЕ атрибутов (CSS поверх разметки).
    let css = |l: Option<crate::value::Len>| match l {
        Some(crate::value::Len::Px(v)) => Some(v),
        _ => None,
    };
    if let (Some(w), Some(h)) = (
        css(e.style.width).or_else(|| num("width")),
        css(e.style.height).or_else(|| num("height")),
    ) {
        return (w, h);
    }
    if let Some(vb) = e.attr("viewBox") {
        let p: Vec<f32> = vb
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        if p.len() == 4 && p[2] > 0.0 && p[3] > 0.0 {
            return (p[2], p[3]);
        }
    }
    (120.0, 120.0)
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
                    px_len(el.style.width)
                        .or_else(|| el.attr("width").and_then(|v| v.parse().ok()))
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
    let visible_y = !contained
        && e.style.overflow_y == Some(crate::computed::Overflow::Visible);
    let visible_x = !contained
        && e.style.overflow_x == Some(crate::computed::Overflow::Visible);
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
    let image = rasterize(&serialize_sized(e, rw, rh), rw, rh)?;
    let mut img = gpui::img(ImageSource::Render(image))
        .w(gpui::px(rw))
        .h(gpui::px(rh));
    // CSS-фон самого <svg> (`svg { background: green }`): канва растра
    // прозрачна, фон красится коробкой картинки (svg-scale-001 и родня).
    if let Some(bg) = e.style.background {
        use gpui::Styled as _;
        img = img.bg(bg.to_hsla());
    }
    if rw > w + 0.5 || rh > h + 0.5 {
        Some(
            {
                use gpui::ParentElement as _;
                use gpui::Styled as _;
                gpui::div()
                    .w(gpui::px(w))
                    .h(gpui::px(h))
                    .flex_shrink_0()
                    .child(img.absolute().top_0().left_0())
                    .into_any_element()
            },
        )
    } else {
        Some(img.into_any_element())
    }
}

/// Разметка с корневой канвой ЗАДАННОГО размера: собственные width/height
/// корня заменяются, иначе растеризатор вписал бы рисунок по ним.
fn serialize_sized(e: &Element, w: f32, h: f32) -> String {
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
    out.push_str(&format!("<svg width=\"{w}\" height=\"{h}\""));
    for (k, v) in &e.attrs {
        if k == "width" || k == "height" {
            continue;
        }
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        escape_attr(v, &mut out);
        out.push('"');
    }
    out.push('>');
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

        let nodes = parse(r#"<svg viewBox="0 0 64 32"></svg>"#, "");
        assert_eq!(size_of(find_svg(&nodes).unwrap()), (64.0, 32.0));
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
