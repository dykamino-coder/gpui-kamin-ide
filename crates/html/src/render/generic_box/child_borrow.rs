//! Identify child lists whose layout preparation can use the DOM directly.

use crate::dom::{Element, Node};
use crate::style::computed::Computed;

pub(super) fn borrowed<'a>(element: &'a Element, merged: &Computed) -> Option<&'a [Node]> {
    if element.style.grid_areas.is_some() || merged.vertical == Some(true) {
        return None;
    }
    // Horizontal preparation only adjusts non-inline vertical children.
    if element.children.iter().any(|node| {
        matches!(node, Node::Element(child) if !child.inline && child.style.vertical == Some(true))
    }) {
        return None;
    }
    Some(&element.children)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_flow_borrows_the_original_subtree() {
        let child =
            crate::layout::table::anon::anon_element("div", vec![Node::Text("text".into())]);
        let mut parent =
            crate::layout::table::anon::anon_element("div", vec![Node::Element(child)]);
        let merged = Computed::default();
        let children = borrowed(&parent, &merged).unwrap();
        assert!(std::ptr::eq(children.as_ptr(), parent.children.as_ptr()));
        let Node::Element(child) = &mut parent.children[0] else {
            unreachable!()
        };
        child.inline = false;
        child.style.vertical = Some(true);
        assert!(borrowed(&parent, &merged).is_none());
    }

    #[test]
    fn grid_and_vertical_flow_keep_the_preparation_path() {
        let mut parent = crate::layout::table::anon::anon_element("div", vec![]);
        let mut merged = Computed {
            vertical: Some(true),
            ..Computed::default()
        };
        assert!(borrowed(&parent, &merged).is_none());
        merged.vertical = None;
        parent.style.grid_areas = Some(vec![vec!["area".into()]]);
        assert!(borrowed(&parent, &merged).is_none());
    }
}
