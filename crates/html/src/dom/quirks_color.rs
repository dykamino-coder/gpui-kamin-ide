//! Quirks Mode §3.12: only HTML tables with initial color use the body color.

use crate::dom::Node;
use crate::style::computed::Computed;
use crate::style::values::value::Color;

pub(super) fn resolve(nodes: &mut [Node]) {
    let Some(Node::Element(root)) = nodes.iter().find(|n| matches!(n, Node::Element(_))) else {
        return;
    };
    if root.tag != "html" {
        return;
    }
    let parent =
        crate::style::cascade::inherit::inherit_unpainted(&Computed::default(), &root.style);
    let Some(body) = root.children.iter().find_map(|node| match node {
        Node::Element(e) if e.tag == "body" => Some(e),
        _ => None,
    }) else {
        return;
    };
    let color = crate::style::cascade::inherit::inherit_unpainted(&parent, &body.style)
        .color
        .unwrap_or_else(|| Color::parse("black").unwrap());
    apply(nodes, color);
}

fn apply(nodes: &mut [Node], color: Color) {
    for node in nodes {
        let Node::Element(e) = node else { continue };
        if e.tag == "table" && e.style.color_initial {
            e.style.color = Some(color);
        }
        apply(&mut e.children, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(nodes: &[Node], tag: &str, parent: &Computed) -> Option<Color> {
        nodes.iter().find_map(|node| {
            let Node::Element(e) = node else { return None };
            let computed = crate::style::cascade::inherit::inherit_unpainted(parent, &e.style);
            if e.tag == tag {
                computed.color
            } else {
                color(&e.children, tag, &computed)
            }
        })
    }

    #[test]
    fn initial_table_color_uses_body_but_other_initial_colors_do_not() {
        let doc = "<style>body{color:green}div{color:red}p{color:initial}</style><div><table><tr><td>X</table><p>X";
        let nodes = crate::dom::parse(doc, "");
        assert_eq!(
            color(&nodes, "td", &Computed::default()),
            Color::parse("green")
        );
        assert_eq!(
            color(&nodes, "p", &Computed::default()),
            Color::parse("black")
        );
        let standard = crate::dom::parse(&format!("<!doctype html>{doc}"), "");
        assert_eq!(
            color(&standard, "td", &Computed::default()),
            Color::parse("red")
        );
    }

    #[test]
    fn author_inherited_colors_override_the_table_quirk() {
        for value in ["inherit", "currentColor", "unset", "red"] {
            let doc = format!(
                "<style>body{{color:green}}div{{color:red}}table{{color:{value}}}</style><div><table><tr><td>X"
            );
            let nodes = crate::dom::parse(&doc, "");
            assert_eq!(
                color(&nodes, "td", &Computed::default()),
                Color::parse("red"),
                "{value}"
            );
        }
    }
}
