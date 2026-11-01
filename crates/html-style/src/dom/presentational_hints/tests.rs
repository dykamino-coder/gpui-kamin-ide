//! Проверки каскада представленческих подсказок.

use crate::dom::{Element, Node};
use crate::style::values::value::Len;

#[test]
fn column_attributes_participate_below_author_width() {
    for (html, expected) in [
        ("<table><col width='18junk'></table>", Some(Len::Px(18.0))),
        ("<table><col width='25%'></table>", Some(Len::Pct(0.25))),
        (
            "<style>col{width:auto}</style><table><col width='18'></table>",
            Some(Len::Auto),
        ),
        ("<table><col width='-18'></table>", None),
    ] {
        let nodes = crate::dom::parse(html, "");
        assert_eq!(
            find(&nodes, "col")
                .unwrap()
                .style
                .cascaded()
                .reset_box_layout
                .width,
            expected
        );
    }
}

fn find<'a>(nodes: &'a [Node], tag: &str) -> Option<&'a Element> {
    nodes.iter().find_map(|node| match node {
        Node::Element(e) if e.tag == tag => Some(e),
        Node::Element(e) => find(&e.children, tag),
        _ => None,
    })
}

#[test]
fn table_integer_hints_participate_below_author_css() {
    let reference = crate::dom::parse("<!doctype html><table><tr><td></td></tr></table>", "");
    let default_spacing = find(&reference, "table")
        .unwrap()
        .style
        .cascaded()
        .inh
        .table_list
        .border_spacing;
    let sides = find(&reference, "td")
        .unwrap()
        .style
        .cascaded()
        .reset_box_layout
        .padding;
    let default_padding = [sides.top, sides.right, sides.bottom, sides.left];
    for (raw, value) in [
        ("10", Some(10.0)),
        (" +7junk", Some(7.0)),
        ("3.75", Some(3.0)),
        ("-0foo", Some(0.0)),
        ("-4", None),
        ("junk", None),
        ("", None),
    ] {
        let nodes = crate::dom::parse(
            &format!(
                "<!doctype html><table cellpadding='{raw}' cellspacing='{raw}'><tr><td></td></tr></table>"
            ),
            "",
        );
        assert_eq!(
            find(&nodes, "table")
                .unwrap()
                .style
                .cascaded()
                .inh
                .table_list
                .border_spacing,
            value
                .map(|n| (Some(Len::Px(n)), Some(Len::Px(n))))
                .or(default_spacing),
            "{raw}"
        );
        let sides = find(&nodes, "td")
            .unwrap()
            .style
            .cascaded()
            .reset_box_layout
            .padding;
        assert_eq!(
            [sides.top, sides.right, sides.bottom, sides.left],
            value.map_or(default_padding, |n| [Some(Len::Px(n)); 4]),
            "{raw}"
        );
    }
    let nodes = crate::dom::parse(
        "<!doctype html><style>table{border-spacing:2px}td{padding:1px;padding-left:4px}</style><table cellspacing=10 cellpadding=10><tr><td></td></tr></table>",
        "",
    );
    assert_eq!(
        find(&nodes, "table")
            .unwrap()
            .style
            .cascaded()
            .inh
            .table_list
            .border_spacing,
        Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))))
    );
    let sides = find(&nodes, "td")
        .unwrap()
        .style
        .cascaded()
        .reset_box_layout
        .padding;
    assert_eq!(
        [sides.top, sides.right, sides.bottom, sides.left],
        [
            Some(Len::Px(1.0)),
            Some(Len::Px(1.0)),
            Some(Len::Px(1.0)),
            Some(Len::Px(4.0))
        ]
    );
}

mod flow;
