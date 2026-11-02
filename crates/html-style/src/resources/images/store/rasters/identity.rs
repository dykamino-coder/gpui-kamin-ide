//! Полная идентичность SVG; локальные имена стираются только в замкнутой статической сцене.
//! Исходная разметка остаётся входом растеризатора, нормализуется только ключ кэша.

use std::{collections::HashMap, sync::Arc};
#[cfg(test)]
#[path = "identity_tests.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Identity {
    local: bool,
    pub(super) markup: Arc<str>,
}

impl Identity {
    pub(super) fn new(markup: &str) -> Self {
        let normalized = closed_local_ids(markup);
        Self {
            local: normalized.is_some(),
            markup: normalized.as_deref().unwrap_or(markup).into(),
        }
    }
}

fn token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
}

fn closed_local_ids(markup: &str) -> Option<String> {
    // Крупные/сложные документы сохраняют точный ключ без дополнительного разбора.
    if markup.len() > 256 * 1024 || !markup.contains("id") {
        return None;
    }
    let xml = roxmltree::Document::parse_with_options(
        markup,
        roxmltree::ParsingOptions {
            nodes_limit: 10_000,
            ..Default::default()
        },
    )
    .ok()?;
    let mut ids = HashMap::new();
    for node in xml.descendants() {
        if node.is_pi() {
            return None;
        }
        if !node.is_element() {
            continue;
        }
        if node.tag_name().namespace() != Some("http://www.w3.org/2000/svg")
            || !static_element(node.tag_name().name())
        {
            return None;
        }
        if let Some(id) = node.attribute("id") {
            let index = ids.len();
            if !token(id) || ids.insert(id, index).is_some() {
                return None;
            }
        }
    }
    if ids.is_empty() {
        return None;
    }
    let mut replacements = Vec::new();
    for node in xml.descendants().filter(|n| n.is_element()) {
        for attr in node.attributes() {
            let name = attr.name();
            let value = attr.value();
            if attr.namespace().is_some()
                && !(attr.namespace() == Some("http://www.w3.org/1999/xlink") && name == "href")
            {
                return None;
            }
            if name == "style" || name.starts_with("on") {
                return None;
            }
            let replacement = if name == "id" {
                Some(format!("_svg_{}", ids.get(value)?))
            } else if name == "href" {
                Some(format!("#_svg_{}", ids.get(value.strip_prefix('#')?)?))
            } else if iri_attribute(name) {
                let v = value.trim();
                if let Some(id) = v.strip_prefix("url(#").and_then(|v| v.strip_suffix(')')) {
                    Some(value.replace(
                        &format!("url(#{id})"),
                        &format!("url(#_svg_{})", ids.get(id)?),
                    ))
                } else {
                    // Не разбираем здесь CSS escapes, кавычки, списки и внешние URL.
                    // Они остаются в точном ключе и обрабатываются только usvg.
                    if v.to_ascii_lowercase().contains("url") || v.contains('\\') {
                        return None;
                    }
                    None
                }
            } else {
                None
            };
            if let Some(value) = replacement {
                replacements.push((attr.range_value(), value));
            }
        }
    }
    let mut normalized = markup.to_owned();
    for (range, value) in replacements.into_iter().rev() {
        normalized.replace_range(range, &value);
    }
    Some(normalized)
}

fn iri_attribute(name: &str) -> bool {
    matches!(
        name,
        "fill"
            | "stroke"
            | "filter"
            | "clip-path"
            | "mask"
            | "marker"
            | "marker-start"
            | "marker-mid"
            | "marker-end"
            | "cursor"
            | "color-profile"
    )
}

fn static_element(name: &str) -> bool {
    matches!(
        name,
        "svg"
            | "g"
            | "defs"
            | "symbol"
            | "use"
            | "switch"
            | "title"
            | "desc"
            | "path"
            | "rect"
            | "circle"
            | "ellipse"
            | "line"
            | "polyline"
            | "polygon"
            | "text"
            | "tspan"
            | "textPath"
            | "clipPath"
            | "mask"
            | "pattern"
            | "marker"
            | "linearGradient"
            | "radialGradient"
            | "stop"
            | "filter"
            | "feBlend"
            | "feColorMatrix"
            | "feComponentTransfer"
            | "feComposite"
            | "feConvolveMatrix"
            | "feDiffuseLighting"
            | "feDisplacementMap"
            | "feDistantLight"
            | "feDropShadow"
            | "feFlood"
            | "feFuncA"
            | "feFuncB"
            | "feFuncG"
            | "feFuncR"
            | "feGaussianBlur"
            | "feImage"
            | "feMerge"
            | "feMergeNode"
            | "feMorphology"
            | "feOffset"
            | "fePointLight"
            | "feSpecularLighting"
            | "feSpotLight"
            | "feTile"
            | "feTurbulence"
    )
}
