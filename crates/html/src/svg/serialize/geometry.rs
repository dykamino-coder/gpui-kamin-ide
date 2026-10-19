//! SVG 2 geometry properties from the CSS cascade: `cx`, `cy`, `r`, `rx`, `ry`, `d`.
//!
//! The rasterizer sees only serialized markup, so a cascaded geometry property is
//! written as the attribute of the same name and REPLACES the element's own
//! attribute: the attribute is only a presentation hint, any author rule wins
//! (SVG 2 §9.1–9.4, Paths §9.3 `d`; Blink maps the attributes to
//! `CSSPropertyID::kCx`… in svg_circle_element.cc, svg_ellipse_element.cc,
//! svg_rect_element.cc:63-69, svg_path_element.cc and reads ComputedStyle).

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Record a cascaded geometry declaration in attribute syntax.
pub(crate) fn geometry_property(c: &mut Computed, key: &str, v: &str, vcx: ValueCx<'_>) {
    let name: &'static str = match key {
        "cx" => "cx",
        "cy" => "cy",
        "r" => "r",
        "rx" => "rx",
        "ry" => "ry",
        "d" => "d",
        _ => return,
    };
    let Some(value) = attribute_value(name, v, vcx) else {
        return;
    };
    c.accept_declaration();
    c.svg_geometry.retain(|(n, _)| *n != name);
    c.svg_geometry.push((name, value));
}

/// `d: path("…")` carries the path data, `d: none` is an empty path; lengths
/// are absolute px or percentages, `auto` stays a keyword the rasterizer
/// treats as "not set" (rx/ry `auto` take the other radius).
fn attribute_value(name: &str, v: &str, vcx: ValueCx<'_>) -> Option<String> {
    let v = v.trim();
    if name == "d" {
        if v.eq_ignore_ascii_case("none") {
            return Some(String::new());
        }
        let inner = v.strip_prefix("path(")?.strip_suffix(')')?.trim();
        let data = inner
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .or_else(|| inner.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))?;
        return Some(data.to_string());
    }
    if v.eq_ignore_ascii_case("auto") {
        return Some("auto".to_string());
    }
    match Len::parse(v, vcx)? {
        Len::Px(px) => Some(format!("{px}")),
        Len::Pct(f) => Some(format!("{}%", f * 100.0)),
        _ => None,
    }
}

/// Whether the geometry property `name` applies to the element `tag`.
fn applies(tag: &str, name: &str) -> bool {
    let tag = tag.to_ascii_lowercase();
    match name {
        "cx" | "cy" => matches!(tag.as_str(), "circle" | "ellipse"),
        "r" => tag == "circle",
        "rx" | "ry" => matches!(tag.as_str(), "rect" | "ellipse"),
        "d" => tag == "path",
        _ => false,
    }
}

/// The cascaded value of the attribute `name` of `e`, if a rule sets it.
pub(super) fn css_value<'a>(e: &'a Element, name: &str) -> Option<&'a str> {
    if !applies(&e.tag, name) {
        return None;
    }
    e.style
        .svg_geometry
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.as_str())
}

/// Write the cascaded geometry of `e` as attributes.
pub(super) fn write_geometry(e: &Element, out: &mut String) {
    for (name, value) in &e.style.svg_geometry {
        if applies(&e.tag, name) {
            out.push(' ');
            out.push_str(name);
            out.push_str("=\"");
            super::escape_attr(value, out);
            out.push('"');
        }
    }
}
