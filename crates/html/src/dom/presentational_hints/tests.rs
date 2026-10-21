//! Проверки каскада представленческих подсказок.

use crate::dom::{Element, Node};
use crate::style::computed::Computed;
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
        assert_eq!(find(&nodes, "col").unwrap().style.width, expected);
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
    let default_spacing = find(&reference, "table").unwrap().style.border_spacing;
    let sides = find(&reference, "td").unwrap().style.padding;
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
            find(&nodes, "table").unwrap().style.border_spacing,
            value
                .map(|n| (Some(Len::Px(n)), Some(Len::Px(n))))
                .or(default_spacing),
            "{raw}"
        );
        let sides = find(&nodes, "td").unwrap().style.padding;
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
        find(&nodes, "table").unwrap().style.border_spacing,
        Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))))
    );
    let sides = find(&nodes, "td").unwrap().style.padding;
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

#[test]
fn author_resets_and_nearest_table_control_cell_hints() {
    let nodes = crate::dom::parse(
        "<!doctype html><style>table{border-spacing:initial}td{padding-top:initial}</style><table cellspacing=10 cellpadding=10><tr><td></td></tr></table>",
        "",
    );
    let spacing = find(&nodes, "table").unwrap().style.border_spacing;
    assert_eq!(spacing, Computed::default().border_spacing);
    let padding = find(&nodes, "td").unwrap().style.padding;
    // css-box-4 §4.1: the initial value of a padding side is 0.
    assert_eq!(padding.top, Some(Len::Px(0.0)));
    assert_eq!(padding.bottom, Some(Len::Px(10.0)));

    let nodes = crate::dom::parse(
        "<!doctype html><table cellpadding=10><tr><td><table cellpadding=3><tr><th></th></tr></table></td></tr></table>",
        "",
    );
    assert_eq!(
        find(&nodes, "td").unwrap().style.padding.top,
        Some(Len::Px(10.0))
    );
    assert_eq!(
        find(&nodes, "th").unwrap().style.padding.top,
        Some(Len::Px(3.0))
    );
}
