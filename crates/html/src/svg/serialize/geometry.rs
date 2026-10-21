//! SVG 2 geometry properties from the CSS cascade: `cx`, `cy`, `r`, `rx`, `ry`, `d`, experimental `path-length`.
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

/// Whether the geometry property `name` applies to the element `tag`.
fn applies(tag: &str, name: &str) -> bool {
    let tag = tag.to_ascii_lowercase();
    match name {
        "cx" | "cy" => matches!(tag.as_str(), "circle" | "ellipse"),
        "r" => tag == "circle",
        "rx" | "ry" => matches!(tag.as_str(), "rect" | "ellipse"),
        "d" => tag == "path",
        "pathLength" => matches!(
            tag.as_str(),
            "path" | "rect" | "circle" | "ellipse" | "line" | "polyline" | "polygon"
        ),
        _ => false,
    }
}

/// The cascaded value of the attribute `name` of `e`, if a rule sets it.
pub(super) fn css_value<'a>(e: &'a Element, name: &str) -> Option<&'a str> {
    if !applies(&e.tag, name) {
        return None;
    }
    // pathLength enters the cascade as a presentation hint; a CSS-wide reset
    // clears that hint as well as an earlier author declaration.
    if name == "pathLength" {
        return Some("");
    }
    e.style
        .svg_geometry
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.as_str())
}

/// Write the cascaded geometry of `e` as attributes.
pub(super) fn write_geometry(e: &Element, out: &mut String) {
    if applies(&e.tag, "pathLength") {
        match e.style.svg_path_length {
            Some(Len::Px(value)) => out.push_str(&format!(" pathLength=\"{value}\"")),
            // CSS none removes the presentation hint and disables calibration.
            Some(Len::Auto) => {}
            _ => {}
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(nodes: &'a [crate::dom::Node], tag: &str) -> &'a Element {
        find_optional(nodes, tag).expect("element")
    }

    fn find_optional<'a>(nodes: &'a [crate::dom::Node], tag: &str) -> Option<&'a Element> {
        nodes.iter().find_map(|node| match node {
            crate::dom::Node::Element(e) if e.tag == tag => Some(e),
            crate::dom::Node::Element(e) => find_optional(&e.children, tag),
            _ => None,
        })
    }

    #[test]
    fn css_author_length_replaces_attribute_and_invalid_declarations_preserve_it() {
        let e = crate::dom::parse(
            "<style>path { path-length: 10px; path-length: -1px }</style><svg><path pathLength='100'/></svg>",
            "",
        );
        let out = super::super::serialize(find(&e, "svg"), e.values());
        assert!(out.contains("pathLength=\"10\""), "{out}");
        assert!(!out.contains("pathLength=\"100\""), "{out}");
    }

    #[test]
    fn computed_units_explicit_inheritance_none_and_initial() {
        for (css, expected) in [
            (
                "g {font-size:20px;path-length:2em} path {font-size:10px;path-length:inherit}",
                Some(40.0),
            ),
            ("g {path-length:20px} path {path-length:initial}", None),
            ("g {path-length:20px} path {path-length:unset}", None),
            ("g {path-length:20px}", Some(100.0)),
            (
                "path {font-size:20px;path-length:calc(1em + 2px)}",
                Some(22.0),
            ),
            (
                "path {font-size:20px;line-height:30px;path-length:calc(1em + 1lh)}",
                Some(50.0),
            ),
            ("path {path-length:0}", Some(0.0)),
            ("path {path-length:calc(-1px)}", Some(0.0)),
            ("path {path-length:none}", None),
        ] {
            let mut parsed = crate::dom::parse(
                &format!("<style>{css}</style><svg><g><path pathLength='100'/></g></svg>"),
                "",
            );
            crate::style::cascade::inherited_value::settle(&mut parsed.nodes, parsed.doc.values());
            let out = super::super::serialize(find(&parsed, "svg"), parsed.values());
            if let Some(value) = expected {
                assert!(
                    out.contains(&format!("pathLength=\"{value}\"")),
                    "{css}: {out}"
                );
            } else {
                assert!(!out.contains("pathLength="), "{css}: {out}");
            }
        }
    }
}
