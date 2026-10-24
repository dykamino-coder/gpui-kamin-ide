//! Сериализация каскадных геометрических значений SVG в атрибуты.

use crate::dom::Element;
use crate::style::values::value::Len;

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
        .cascaded()
        .svg_geometry
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.as_str())
}

/// Write the cascaded geometry of `e` as attributes.
pub(super) fn write_geometry(e: &Element, out: &mut String) {
    if applies(&e.tag, "pathLength") {
        match e.style.used().svg_path_length {
            Some(Len::Px(value)) => out.push_str(&format!(" pathLength=\"{value}\"")),
            // CSS none removes the presentation hint and disables calibration.
            Some(Len::Auto) => {}
            _ => {}
        }
    }
    for (name, value) in &e.style.used().svg_geometry {
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
            let parsed = crate::dom::parse(
                &format!("<style>{css}</style><svg><g><path pathLength='100'/></g></svg>"),
                "",
            );
            crate::style::cascade::materialize::materialize(
                &parsed.nodes,
                &std::rc::Rc::new(crate::style::computed::Computed::default()),
                parsed.doc.values(),
            );
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
