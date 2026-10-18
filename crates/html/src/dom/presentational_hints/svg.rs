//! SVG foreignObject presentation attributes mapped into the HTML cascade.

use super::hint;
use crate::style::css::Rule;

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
