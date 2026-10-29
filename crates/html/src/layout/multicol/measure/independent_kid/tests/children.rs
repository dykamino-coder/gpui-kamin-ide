//! Мера детей стопки колонок раскладкой копии: таблица и вложенные колонки.

use super::*;

#[test]
fn table_rows_and_cells_come_from_the_laid_copy() {
    use crate::dom::Node;
    use crate::layout::multicol::measure::measure_children;

    // An out-of-flow sibling keeps the stack off the all-native path; the
    // table still gets its rows, spans and cell content from layout.
    let tree = crate::document::parsed::parse(
        "<div><div style='position:absolute'></div><table style='border-spacing:0'>\
         <tr><td style='break-inside:avoid'><div style='height:50px'></div></td></tr>\
         <tr><td rowspan=2><div style='height:70px'></div></td></tr><tr></tr></table></div>",
        "",
    );
    let host = host(&tree);
    let table = host
        .children
        .iter()
        .find_map(|n| match n {
            Node::Element(e) if e.tag == "table" => Some(e),
            _ => None,
        })
        .expect("table");
    assert!(gate(table, true, Some(50.0)));
    assert!(
        !gate(table, false, Some(50.0)),
        "no layout measure in this stack"
    );
    let opts = opts();
    let measured: RefCell<Vec<(u64, f32)>> = RefCell::default();
    for (whole_ok, measure_ok) in [(true, false), (false, true)] {
        measured.borrow_mut().clear();
        let kids = measure_children(
            host,
            &BoxStyle::default(),
            &opts,
            None,
            None,
            None,
            (Some(50.0), Some(50.0)),
            whole_ok,
            &RefCell::default(),
            &measured,
            false,
            false,
            measure_ok,
            true,
        )
        .expect("table child");
        assert_eq!(kids.len(), 1);
        assert_eq!(*measured.borrow(), vec![(table.node_id, 50.0)]);
    }
}

#[test]
fn nested_columns_preserve_the_roots_fragmentation_dispatch() {
    use crate::dom::Node;
    use crate::layout::multicol::measure::measure_children;

    for (style, independent, paginate) in [
        ("", true, false),
        ("display:block", true, false),
        ("display:list-item", true, false),
        ("display:flex;flex-direction:column", false, false),
        ("display:flex;flex-direction:row", false, false),
        ("display:grid", false, false),
        ("display:flex;flex-direction:column", false, true),
        ("display:flex;flex-direction:row", false, true),
    ] {
        // Анонимный flex-элемент не входит в flex_tree; атомарные колонки
        // внутри соседнего блока допускают DOM-пагинацию этого flex.
        let (before, after, columns_display, tag) = if paginate {
            ("anonymous<div>", "</div>", "display:inline-block;", "span")
        } else {
            ("", "", "", "div")
        };
        let html = format!(
            "<div><div style='{style}'>{before}\
             <{tag} style='{columns_display}columns:2;column-gap:0'>\
             <div style='height:30px'></div><div style='height:50px'></div>\
             </{tag}>{after}</div></div>"
        );
        let tree = crate::document::parsed::parse(&html, "");
        let host = host(&tree);
        let Node::Element(root) = &host.children[0] else {
            panic!("formatting root")
        };
        assert_eq!(gate(root, true, Some(80.0)), independent, "{style}");
        assert_eq!(
            own_layout_gate(root, true, Some(80.0)),
            independent,
            "{style}"
        );
        let opts = opts();
        let measured = RefCell::default();
        let kids = measure_children(
            host,
            &BoxStyle::default(),
            &opts,
            None,
            None,
            None,
            (Some(80.0), Some(80.0)),
            false,
            &RefCell::default(),
            &measured,
            false,
            false,
            true,
            true,
        )
        .expect("supported fragmentation subtree");
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].0.marks.paginate, paginate, "{style}");
        // Каждый ребёнок измеряется раскладкой своей копии, как бы его ни
        // фрагментировали дальше.
        assert_eq!(*measured.borrow(), vec![(root.node_id, 80.0)], "{style}");
    }
}
