//! Разбор каскадных геометрических свойств SVG 2: длины, `d: path()` и `path-length`.
//! Значения сохраняются в Computed; SVG-сериализация выбирает применимые атрибуты.

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

mod path_length;

/// Record a cascaded geometry declaration in attribute syntax.
pub(crate) fn geometry_property(c: &mut Computed, key: &str, v: &str, vcx: ValueCx<'_>) {
    if key == "path-length" {
        if let Some(length) = path_length::parse(v, vcx) {
            c.accept_declaration();
            c.svg_path_length = Some(length);
        }
        return;
    }
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
