//! Сериализация атрибутов геометрии, заливки и обводки SVG.

use super::escape_attr;
use crate::dom::Element;

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_attributes(
    e: &Element,
    out: &mut String,
    combined: &Option<String>,
    clip_id: &Option<String>,
    attr_bad: bool,
) {
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
}
