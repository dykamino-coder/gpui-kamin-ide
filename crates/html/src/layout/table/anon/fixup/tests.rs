//! Тесты: починка детей таблицы и растворённые `display: contents`.

use super::*;
use crate::style::values::value::Len;

#[test]
fn dissolved_contents_hand_down_their_computed_font_size() {
    let parsed = crate::dom::parse(
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
            Node::Element(e) if e.style.used().display == Some(Display::Table) => Some(e),
            Node::Element(e) => table(&e.children),
            _ => None,
        })
    }
    let table = table(&parsed.nodes).expect("table");
    let flat = flatten_contents(&table.children, vcx);
    let row = flat
        .iter()
        .find_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
        .expect("row");
    assert_eq!(row.style.used().font_size, Some(Len::Px(30.0)));
}
