//! Authored display overrides the default table-cell role of td and th.
use crate::computed::Display;
use crate::dom::Element;

pub(super) fn is_cell(element: &Element) -> bool {
    match element.style.display {
        Some(Display::TableCell) => true,
        None => matches!(element.tag.as_str(), "td" | "th"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Node;

    #[test]
    fn authored_display_controls_cell_role_before_anonymous_fixup() {
        let mut cell = super::super::anon_element("td", vec![]);
        assert!(super::super::is_cell(&cell));
        cell.style.display = Some(Display::Block);
        assert!(!super::super::is_cell(&cell));
        cell.style.display = Some(Display::TableCell);
        assert!(super::super::is_cell(&cell));
        cell.tag = "div".into();
        assert!(super::super::is_cell(&cell));
    }

    #[test]
    fn block_td_is_content_of_an_anonymous_cell_and_retains_its_style() {
        let mut block = super::super::anon_element("td", vec![Node::Text("data".into())]);
        block.style.display = Some(Display::Block);
        block.node_id = 123;
        let row = super::super::anon_element("tr", vec![Node::Element(block)]);
        let fixed = super::super::fixup_row_children(&row);
        let Node::Element(row) = &fixed[0] else {
            panic!("row")
        };
        let Node::Element(cell) = &row.children[0] else {
            panic!("anonymous cell")
        };
        assert!(super::super::is_cell(cell));
        assert_eq!(cell.style.display, None);
        let Node::Element(block) = &cell.children[0] else {
            panic!("authored block")
        };
        assert_eq!(block.node_id, 123);
        assert_eq!(block.style.display, Some(Display::Block));
        assert_eq!(block.children.len(), 1);
    }
}
