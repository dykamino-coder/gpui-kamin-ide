//! Ссылки растров SVG: разбор XML и привязка к адресу самого рисунка.

use crate::resources::loader::ResourceUrl;
use std::ops::Range;
use url::Url;

fn resource_links(markup: &str) -> Vec<(String, Range<usize>)> {
    let options = roxmltree::ParsingOptions {
        // usvg принимает DTD; привязка базы должна разбирать те же SVG.
        allow_dtd: true,
        ..Default::default()
    };
    let Ok(xml) = roxmltree::Document::parse_with_options(markup, options) else {
        return Vec::new();
    };
    xml.descendants()
        .filter(|n| matches!(n.tag_name().name(), "image" | "feImage" | "use"))
        .flat_map(|n| {
            n.attributes()
                .filter(|a| a.name() == "href")
                .map(|a| (a.value().to_string(), a.range_value()))
        })
        .collect()
}

/// Меняем только значения внешних href, сохраняя CSS и остальную разметку.
pub(crate) fn with_base(markup: &str, base: Option<&Url>) -> String {
    let mut out = markup.to_string();
    for (href, range) in resource_links(markup).into_iter().rev() {
        if href.starts_with('#') || href.starts_with("data:") {
            continue;
        }
        let (url, fragment) = ResourceUrl::resolve(&href, base);
        let mut value = url.as_str().to_string();
        if let Some(fragment) = fragment {
            value.push('#');
            value.push_str(&fragment);
        }
        let value = value
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
            .replace('<', "&lt;");
        out.replace_range(range, &value);
    }
    out
}
