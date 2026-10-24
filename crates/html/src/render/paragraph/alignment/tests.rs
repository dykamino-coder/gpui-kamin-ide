//! Тесты: атом встаёт в строку по своему `vertical-align`, а не по значению строчного предка.

use super::*;
use crate::text::paragraph::AtomAlign;
use std::rc::Rc;

fn find<'a>(nodes: &'a [Node], tag: &str) -> Option<&'a Element> {
    nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.tag == tag => Some(e),
        Node::Element(e) => find(&e.children, tag),
        _ => None,
    })
}

#[test]
fn an_atom_takes_its_own_vertical_align_not_the_inline_ancestors() {
    let parsed = crate::dom::parse(
        r#"<body><span style="vertical-align: top">a <b style="display: inline-block">x</b> <i style="display: inline-block; vertical-align: bottom">y</i></span></body>"#,
        "",
    );
    let vcx = parsed.values();
    crate::style::cascade::materialize::materialize(
        &parsed.nodes,
        &Rc::new(Computed::default()),
        vcx,
    );
    let opts = RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let span = find(&parsed.nodes, "span").unwrap();
    let parent = BoxStyle::shared(span.style.computed());
    // `vertical-align` is not inherited (CSS 2.1 §10.8.1): the atom inside a
    // `top` span sits on the baseline.
    let b = find(&parsed.nodes, "b").unwrap();
    assert_eq!(
        atom_line_align(b, &parent, &opts),
        Some(AtomAlign::Shift(0.0))
    );
    let i = find(&parsed.nodes, "i").unwrap();
    assert_eq!(atom_line_align(i, &parent, &opts), Some(AtomAlign::Bottom));
}
