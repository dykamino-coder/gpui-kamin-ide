//! CSS color inheritance and presentation attributes survive SVG isolation.

use crate::dom::{Element, Node};

fn svg(nodes: &[Node]) -> &Element {
    nodes
        .iter()
        .find_map(|node| match node {
            Node::Element(e) if e.tag == "svg" => Some(e),
            Node::Element(e) => find_svg(&e.children),
            Node::Text(_) => None,
        })
        .expect("svg")
}

fn find_svg(nodes: &[Node]) -> Option<&Element> {
    nodes.iter().find_map(|node| match node {
        Node::Element(e) if e.tag == "svg" => Some(e),
        Node::Element(e) => find_svg(&e.children),
        Node::Text(_) => None,
    })
}

fn outputs(html: &str, css: &str) -> [String; 2] {
    let parsed = crate::dom::parse(html, css);
    crate::style::cascade::materialize::materialize(
        &parsed.nodes,
        &std::rc::Rc::new(crate::style::computed::Computed::default()),
        parsed.values(),
    );
    let root = svg(&parsed.nodes);
    [
        super::serialize(root, parsed.values()),
        super::serialize_viewport(root, (100.0, 50.0), 100.0, 50.0, 0.0, 0.0, parsed.values()),
    ]
}

#[test]
fn inherited_html_color_is_exported_once_in_both_svg_paths() {
    for dir in ["ltr", "rtl"] {
        for out in outputs(
            &format!(
                r#"<div dir="{dir}" style="color:rgba(255,0,0,.5)"><svg><g><rect fill="currentColor"/><path stroke="currentColor"/></g></svg></div>"#
            ),
            "",
        ) {
            assert_eq!(out.matches(" color=").count(), 1, "{out}");
            assert!(out.contains(r#"color="rgba(255, 0, 0, 0.5)""#), "{out}");
            assert!(out.contains(r#"fill="currentColor""#), "{out}");
            assert!(out.contains(r#"stroke="currentColor""#), "{out}");
        }
    }
}

#[test]
fn presentation_color_and_css_priority_are_preserved() {
    for out in outputs(
        r#"<div style="color:red"><svg color="blue" style="color:green"><g color="white"><rect color="inherit" fill="currentColor"/></g><rect color="initial" fill="currentColor"/></svg></div>"#,
        "svg {color:blue!important}",
    ) {
        assert!(out.contains(r#"color="rgba(0, 0, 255, 1)""#), "{out}");
        assert!(
            out.contains(r#"<g color="rgba(255, 255, 255, 1)""#),
            "{out}"
        );
        assert!(out.contains(r#"color="rgba(0, 0, 0, 1)""#), "{out}");
        assert!(!out.contains("color:green"), "{out}");
        assert!(!out.contains(r#"color="inherit""#), "{out}");
    }
}

#[test]
fn html_color_attribute_is_not_an_svg_presentation_hint() {
    for out in outputs(
        r#"<div color="red"><svg><rect fill="currentColor"/></svg></div>"#,
        "",
    ) {
        assert!(!out.contains(" color="), "{out}");
    }
}

#[test]
fn embedded_styles_cannot_recascade_computed_color_in_the_rasterizer() {
    for out in outputs(
        r#"<div style="color:red"><svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><style>rect {color:red!important;fill:currentColor}</style><rect width="32" height="32" style="color:blue!important"/></svg></div>"#,
        "",
    ) {
        let tree = resvg::usvg::Tree::from_str(&out, &Default::default()).expect(&out);
        let mut pixels = resvg::tiny_skia::Pixmap::new(32, 32).unwrap();
        resvg::render(&tree, Default::default(), &mut pixels.as_mut());
        assert_eq!(pixels.pixel(16, 16).unwrap().blue(), 255, "{out}");
        assert_eq!(pixels.pixel(16, 16).unwrap().red(), 0, "{out}");
    }
}

#[test]
fn color_filter_keeps_nested_rules_custom_values_and_opaque_tokens() {
    let css = r#"@media all {g{c\6flor:red;fill:url('data:image/svg+xml;a:b');--paint:{color:blue};stroke:currentColor}}"#;
    let out = super::color::stylesheet(css);
    assert!(!out.contains(r"c\6flor:red"), "{out}");
    assert!(out.contains("fill:url('data:image/svg+xml;a:b')"), "{out}");
    assert!(out.contains("--paint:{color:blue}"), "{out}");
    assert!(out.contains("stroke:currentColor"), "{out}");
    assert_eq!(out.matches('{').count(), out.matches('}').count());
    assert_eq!(
        super::color::without_inline_color(
            r#"fill:url('data:a;b:c');/*x*/COLOR:red;--x:"color:red""#
        ),
        r#"fill:url('data:a;b:c');--x:"color:red""#,
    );
}
