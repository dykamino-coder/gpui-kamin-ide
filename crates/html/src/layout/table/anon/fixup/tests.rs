//! Тесты: починка детей таблицы и растворённые `display: contents`.

use super::*;
use crate::style::values::value::Len;

#[test]
fn dissolved_contents_hand_down_their_computed_font_size() {
    let parsed = crate::document::parsed::parse(
        r#"<div style="display: table; font-size: 20px"><div style="display: contents; font-size: 150%"><div style="display: table-row"><div style="display: table-cell">a</div></div></div></div>"#,
        "",
    );
    let vcx = parsed.values();
    crate::style::cascade::materialize::materialize(
        &parsed.nodes,
        &std::rc::Rc::new(crate::style::computed::Computed::default()),
        vcx,
    );
    fn table(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.style.used().reset_box_layout.display == Some(Display::Table) => {
                Some(e)
            }
            Node::Element(e) => table(&e.children),
            _ => None,
        })
    }
    let table = table(&parsed.nodes).expect("table");
    let flat = flatten_contents(&table.children);
    let row = flat
        .iter()
        .find_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
        .expect("row");
    assert_eq!(row.style.used().inh.font.font_size, Some(Len::Px(30.0)));
}

#[test]
fn row_text_of_uncollapsible_spaces_gets_an_anonymous_cell() {
    // CSS Text 3 §4.1.1: only space, tab, CR and LF collapse; U+00A0, U+3000
    // and form feed are content and need a cell like any other text.
    let cells = |text: &str| {
        let html = format!(
            r#"<div style="display: table"><div style="display: table-row"><div style="display: table-cell">a</div>{text}</div></div>"#
        );
        let parsed = crate::document::parsed::parse(&html, "");
        crate::style::cascade::materialize::materialize(
            &parsed.nodes,
            &std::rc::Rc::new(crate::style::computed::Computed::default()),
            parsed.values(),
        );
        fn row(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e)
                    if e.style.used().reset_box_layout.display == Some(Display::TableRow) =>
                {
                    Some(e)
                }
                Node::Element(e) => row(&e.children),
                _ => None,
            })
        }
        let fixed = fixup_row_children(row(&parsed.nodes).expect("row"));
        let [Node::Element(fixed)] = fixed.as_slice() else {
            panic!("one row");
        };
        fixed
            .children
            .iter()
            .filter(|n| matches!(n, Node::Element(_)))
            .count()
    };
    for content in ["\u{a0}", "\u{3000}", "\u{c}", "x"] {
        assert_eq!(cells(content), 2, "{content:?}");
    }
    assert_eq!(cells(" \t\n"), 1);
}
