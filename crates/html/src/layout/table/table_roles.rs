//! Роль ячейки таблицы: авторский `display` сильнее умолчания `td`/`th`; строчные прогоны
//! в ряду становятся анонимными ячейками до отбрасывания пробелов (CSS 2 §17.2.1
//! удаляет только анонимную строчную коробку из одних пробелов).
use crate::dom::Node;
#[cfg(test)]
use crate::style::computed::Display;

/// CSS 2 section 17.2.1 removes only a whitespace-only anonymous inline
/// box. Leading spaces next to inline content belong to that same box.
pub(super) fn flush_inline(cells: &mut Vec<Node>, run: &mut Vec<Node>) {
    let inline = std::mem::take(run);
    if inline
        .iter()
        .any(|node| !crate::dom::whitespace::is_blank(node))
    {
        cells.push(Node::Element(crate::layout::table::anon::anon_element(
            "td", inline,
        )));
    }
}

pub(super) use crate::dom::table_boxes::is_cell;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_display_controls_cell_role_before_anonymous_fixup() {
        let mut cell = crate::layout::table::anon::anon_element("td", vec![]);
        assert!(crate::layout::table::is_cell(&cell));
        cell.style.used_mut().display = Some(Display::Block);
        assert!(!crate::layout::table::is_cell(&cell));
        cell.style.used_mut().display = Some(Display::TableCell);
        assert!(crate::layout::table::is_cell(&cell));
        cell.tag = "div".into();
        assert!(crate::layout::table::is_cell(&cell));
    }

    #[test]
    fn whitespace_around_inline_content_stays_in_its_anonymous_cell() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let node = |tag, text: &str| {
            Node::Element(crate::layout::table::anon::anon_element(
                tag,
                vec![Node::Text(text.into())],
            ))
        };
        let row = crate::layout::table::anon::anon_element(
            "tr",
            vec![
                node("td", "a"),
                Node::Text(" ".into()),
                node("span", "bc"),
                Node::Text(" ".into()),
                node("td", "d"),
            ],
        );
        let fixed = crate::layout::table::anon::fixup_row_children(&row, vcx);
        let Node::Element(row) = &fixed[0] else {
            panic!("row")
        };
        assert_eq!(row.children.len(), 3);
        let Node::Element(cell) = &row.children[1] else {
            panic!("cell")
        };
        let [
            Node::Text(before),
            Node::Element(content),
            Node::Text(after),
        ] = &cell.children[..]
        else {
            panic!("anonymous inline contents")
        };
        assert_eq!(
            (before.as_str(), content.tag.as_str(), after.as_str()),
            (" ", "span", " ")
        );
    }

    #[test]
    fn block_td_is_content_of_an_anonymous_cell_and_retains_its_style() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut block =
            crate::layout::table::anon::anon_element("td", vec![Node::Text("data".into())]);
        block.style.used_mut().display = Some(Display::Block);
        block.node_id = 123;
        let row = crate::layout::table::anon::anon_element("tr", vec![Node::Element(block)]);
        let fixed = crate::layout::table::anon::fixup_row_children(&row, vcx);
        let Node::Element(row) = &fixed[0] else {
            panic!("row")
        };
        let Node::Element(cell) = &row.children[0] else {
            panic!("anonymous cell")
        };
        assert!(crate::layout::table::is_cell(cell));
        assert_eq!(cell.style.computed().display, None);
        let Node::Element(block) = &cell.children[0] else {
            panic!("authored block")
        };
        assert_eq!(block.node_id, 123);
        assert_eq!(block.style.computed().display, Some(Display::Block));
        assert_eq!(block.children.len(), 1);
    }
}
