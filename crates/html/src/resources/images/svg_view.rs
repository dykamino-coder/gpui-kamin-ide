//! Начальный вид SVG по фрагменту адреса (`svgView(...)`, `xywh=`, `<view id>`) решается один
//! раз, до вычисления собственного размера и растра: правки `viewBox`, `preserveAspectRatio` и
//! `transform` корня. SVG 2 §16.3.2 и Blink `SVGViewSpec` сохраняют незаданные параметры корня
//! и переопределяют только значения из валидного фрагмента или элемента `<view>`.
//!
//! Вид — выбор представления тех же байт рисунка, а не второй запрос: фрагмент не входит в
//! ключ загрузки (`resources::loader::ResourceUrl`).

pub(crate) mod markup;
use markup::{opening_end, xml_text};
use std::ops::Range;

mod spatial;
mod syntax;
use syntax::{valid_box, valid_transform};
#[cfg(test)]
mod tests;

#[derive(Default)]
struct View<'a> {
    view_box: Option<&'a str>,
    aspect: Option<&'a str>,
    transform: Option<&'a str>,
}

/// Разметка рисунка с начальным видом по фрагменту адреса (процентно-кодированному, как
/// в адресе). Не UTF-8, не XML, не `<svg>` или вида нет — байты как есть.
pub(crate) fn apply_view(bytes: Vec<u8>, fragment: &str) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return bytes;
    };
    let Ok(doc) = roxmltree::Document::parse_with_options(text, parsing_options()) else {
        return bytes;
    };
    let decoded = crate::resources::loader::data_url::percent_decode(fragment);
    let Ok(fragment) = std::str::from_utf8(&decoded) else {
        return bytes;
    };
    let edits = edits(text, &doc, fragment);
    if edits.is_empty() {
        return bytes;
    }
    markup::apply(text, edits)
}

/// Разбор рисунка: внутренний DTD допустим (сущности в разметке рисунков набора).
pub(crate) fn parsing_options<'input>() -> roxmltree::ParsingOptions<'input> {
    roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    }
}

/// Правки корня по уже декодированному фрагменту: `viewBox`, `preserveAspectRatio`,
/// `transform`.
pub(crate) fn edits(
    text: &str,
    doc: &roxmltree::Document<'_>,
    fragment: &str,
) -> Vec<(Range<usize>, String)> {
    let root = doc.root_element();
    if root.tag_name().name() != "svg" {
        return Vec::new();
    }
    let spatial = spatial::view_box(text, fragment);
    let view = if fragment.starts_with("svgView(") {
        parse(fragment)
    } else if let Some(ref view_box) = spatial {
        Some(View {
            view_box: Some(view_box),
            ..View::default()
        })
    } else {
        doc.descendants()
            .find(|n| n.is_element() && n.attribute("id") == Some(fragment))
            .filter(|n| n.has_tag_name("view"))
            .map(|n| View {
                view_box: n.attribute("viewBox").filter(|v| valid_box(v)),
                aspect: n
                    .attribute("preserveAspectRatio")
                    .filter(|v| valid_aspect(v)),
                transform: None,
            })
    };
    let Some(view) = view else { return Vec::new() };
    let Some(mut end) = opening_end(text, root.range().start) else {
        return Vec::new();
    };
    if text.as_bytes().get(end.wrapping_sub(1)) == Some(&b'/') {
        end -= 1;
    }
    let mut edits = Vec::new();
    let mut extra = String::new();
    for (name, value) in [
        ("viewBox", view.view_box),
        ("preserveAspectRatio", view.aspect),
        ("transform", view.transform),
    ] {
        let Some(value) = value else { continue };
        let value = if name == "transform" {
            // SVG 2 §8.5: the root transform sits outside the viewBox mapping.
            format!("{} {value}", root.attribute(name).unwrap_or(""))
        } else {
            value.to_string()
        };
        if let Some(attr) = root.attributes().find(|a| a.name() == name) {
            edits.push((attr.range_value(), xml_text(&value)));
        } else {
            extra.push_str(&format!(" {name}=\"{}\"", xml_text(&value)));
        }
    }
    if !extra.is_empty() {
        edits.push((end..end, extra));
    }
    edits
}

fn parse(fragment: &str) -> Option<View<'_>> {
    // Time segments do not change a static image's initial spatial view.
    let mut segments = fragment.split('&');
    let fragment = segments.next()?;
    if segments.any(|segment| !segment.starts_with("t=")) {
        return None;
    }
    let mut rest = fragment.strip_prefix("svgView(")?.strip_suffix(')')?.trim();
    let mut view = View::default();
    while !rest.is_empty() {
        let open = rest.find('(')?;
        let name = &rest[..open];
        let mut depth = 1usize;
        let close = rest[open + 1..].char_indices().find_map(|(i, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(open + 1 + i)
        })?;
        let value = rest[open + 1..close].trim();
        match name {
            "viewBox" if view.view_box.is_none() && valid_box(value) => view.view_box = Some(value),
            "preserveAspectRatio" if view.aspect.is_none() && valid_aspect(value) => {
                view.aspect = Some(value)
            }
            "transform" if view.transform.is_none() && valid_transform(value) => {
                view.transform = Some(value)
            }
            // Retained by Blink and Gecko for older SVG 1.1 content.
            "viewTarget" => {}
            "zoomAndPan" if matches!(value, "disable" | "magnify") => {}
            _ => return None,
        }
        rest = rest[close + 1..].trim();
        if !rest.is_empty() {
            rest = rest.strip_prefix(';')?.trim();
            if rest.is_empty() {
                return None;
            }
        }
    }
    Some(view)
}

fn valid_aspect(value: &str) -> bool {
    let mut parts = value.split_ascii_whitespace();
    let mut align = parts.next();
    if align == Some("defer") {
        align = parts.next();
    }
    matches!(
        align,
        Some(
            "none"
                | "xMinYMin"
                | "xMidYMin"
                | "xMaxYMin"
                | "xMinYMid"
                | "xMidYMid"
                | "xMaxYMid"
                | "xMinYMax"
                | "xMidYMax"
                | "xMaxYMax"
        )
    ) && matches!(parts.next(), None | Some("meet" | "slice"))
        && parts.next().is_none()
}
