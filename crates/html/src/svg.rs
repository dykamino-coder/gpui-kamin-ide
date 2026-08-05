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
    write_element(e, &mut out);
    out
}

fn write_element(e: &Element, out: &mut String) {
    out.push('<');
    out.push_str(&e.tag);
    for (k, v) in &e.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        escape_attr(v, out);
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
    if let (Some(w), Some(h)) = (num("width"), num("height")) {
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
    let image = rasterize(&serialize(e), w, h)?;
    Some(
        gpui::img(ImageSource::Render(image))
            .w(gpui::px(w))
            .h(gpui::px(h))
            .into_any_element(),
    )
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
