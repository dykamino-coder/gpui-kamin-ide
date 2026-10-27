//! Quirks Mode §3.12: таблица с начальным `color` берёт цвет `<body>`.
//!
//! Проход по собранному дереву в режиме quirks: вычисляется цвет тела (с
//! наследованием от корня, умолчание чёрный) и ставится тем `<table>`, у
//! которых `color_initial` — то есть цвет пришёл из `table { color: initial }`
//! листа quirks, а не от авторского `inherit`/`currentColor`/значения.

use crate::document::state::ValueCx;
use crate::dom::Node;
use crate::style::cascade::preparation::Colors;
use crate::style::cascade::preparation::colors;
use crate::style::values::value::Color;

pub(super) fn resolve(nodes: &mut [Node], vcx: ValueCx<'_>) {
    let Some(Node::Element(root)) = nodes.iter().find(|n| matches!(n, Node::Element(_))) else {
        return;
    };
    if root.tag != "html" {
        return;
    }
    let parent = colors(&Colors::default(), &root.style, vcx);
    let Some(body) = root.children.iter().find_map(|node| match node {
        Node::Element(e) if e.tag == "body" => Some(e),
        _ => None,
    }) else {
        return;
    };
    let color = colors(&parent, &body.style, vcx)
        .foreground
        .color
        .unwrap_or(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
    apply(nodes, color);
}

fn apply(nodes: &mut [Node], color: Color) {
    for node in nodes {
        let Node::Element(e) = node else { continue };
        if e.tag == "table" && e.style.cascaded().color_initial {
            e.style.cascaded_mut().color = Some(color);
        }
        apply(&mut e.children, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::cascade::preparation::Colors;

    fn color(
        nodes: &[Node],
        tag: &str,
        parent: &Colors,
        vcx: crate::document::state::ValueCx<'_>,
    ) -> Option<Color> {
        nodes.iter().find_map(|node| {
            let Node::Element(e) = node else { return None };
            let computed = colors(parent, &e.style, vcx);
            if e.tag == tag {
                computed.foreground.color
            } else {
                color(&e.children, tag, &computed, vcx)
            }
        })
    }

    #[test]
    fn initial_table_color_uses_body_but_other_initial_colors_do_not() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let doc = "<style>body{color:green}div{color:red}p{color:initial}</style><div><table><tr><td>X</table><p>X";
        let nodes = crate::dom::parse(doc, "");
        assert_eq!(
            color(&nodes, "td", &Colors::default(), vcx),
            Color::parse("green", vcx)
        );
        assert_eq!(
            color(&nodes, "p", &Colors::default(), vcx),
            Color::parse("black", vcx)
        );
        let standard = crate::dom::parse(&format!("<!doctype html>{doc}"), "");
        assert_eq!(
            color(&standard, "td", &Colors::default(), vcx),
            Color::parse("red", vcx)
        );
    }

    #[test]
    fn author_inherited_colors_override_the_table_quirk() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        for value in ["inherit", "currentColor", "unset", "red"] {
            let doc = format!(
                "<style>body{{color:green}}div{{color:red}}table{{color:{value}}}</style><div><table><tr><td>X"
            );
            let nodes = crate::dom::parse(&doc, "");
            assert_eq!(
                color(&nodes, "td", &Colors::default(), vcx),
                Color::parse("red", vcx),
                "{value}"
            );
        }
    }
}
