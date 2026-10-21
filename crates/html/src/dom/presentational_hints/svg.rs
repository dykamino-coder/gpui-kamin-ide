//! Презентационные атрибуты SVG, чьи CSS-соответствия участвуют в нашем каскаде.

use super::hint;
use crate::style::css::Rule;

/// The experimental mapping uses an SVG number as a CSS pixel length.
pub(super) fn path_length(tag: &str, attrs: &[(String, String)]) -> Option<Rule> {
    if !matches!(
        tag,
        "path" | "rect" | "circle" | "ellipse" | "line" | "polyline" | "polygon"
    ) {
        return None;
    }
    let raw = &attrs.iter().find(|(name, _)| name == "pathLength")?.1;
    let number = raw.trim().parse::<f32>().ok()?;
    if !number.is_finite() || number < 0.0 {
        return None;
    }
    hint(&format!("path-length: {number}px"))
}

/// SVG 2 §6.6 presentation attributes of `<foreignObject>` that style its
/// HTML formatting context (the box and the inherited text properties of its
/// content). Like HTML hints they form specificity-0 author rules ahead of
/// every author sheet. Other SVG elements are painted by the SVG rasterizer,
/// which reads its own attributes.
pub(super) fn foreign_object(tag: &str, attrs: &[(String, String)]) -> Option<Rule> {
    const MAPPED: [&str; 18] = [
        "color",
        "direction",
        "display",
        "font-family",
        "font-size",
        "font-size-adjust",
        "font-stretch",
        "font-style",
        "font-variant",
        "font-weight",
        "letter-spacing",
        "overflow",
        "text-decoration",
        "text-overflow",
        "unicode-bidi",
        "visibility",
        "white-space",
        "word-spacing",
    ];
    if !tag.eq_ignore_ascii_case("foreignObject") {
        return None;
    }
    let css: Vec<String> = attrs
        .iter()
        .filter(|(k, v)| MAPPED.contains(&k.as_str()) && !v.contains([';', '{', '}']))
        .map(|(k, v)| format!("{k}: {v}"))
        .collect();
    if css.is_empty() {
        return None;
    }
    hint(&css.join("; "))
}
