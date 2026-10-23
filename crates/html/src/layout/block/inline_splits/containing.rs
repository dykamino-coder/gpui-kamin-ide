//! Исходная связь CB сохраняется при перестройке дерева коробок (CSS 2 §9.2.1.1).

use crate::dom::{Element, InlineCb};
use crate::style::computed::BoxStyle;

pub(super) fn hoist(block: &mut Element, inline: &Element) {
    if crate::text::inline::establishes_cb(&inline.style.computed()) {
        let old = block.marks.inline_cb;
        block.marks.inline_cb = Some(InlineCb {
            node: old.map_or(inline.node_id, |cb| cb.node),
            depth: old.map_or(1, |cb| cb.depth + 1),
        });
    }
}

/// Геометрия продолжения не меняет исходного CSS containing block.
pub(crate) fn restore_containing_block(e: &Element, own: &mut BoxStyle) {
    if let Some(cb) = e.marks.inline_cb {
        own.cb_node = cb.node;
        own.cb_ancestor = true;
        own.cb_depth += cb.depth;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Node;
    use crate::style::computed::{Computed, Position};

    #[test]
    fn nested_inline_splits_keep_the_nearest_original_cb() {
        let block = Element::synthetic("div", Computed::default(), Default::default(), vec![]);
        let style = Computed {
            position: Some(Position::Relative),
            ..Default::default()
        };
        let mut inner = Element::synthetic(
            "span",
            style,
            Default::default(),
            vec![Node::Element(block)],
        );
        inner.node_id = 12;
        inner.inline = true;
        let mut outer = inner.clone();
        outer.node_id = 11;
        outer.children = vec![Node::Element(inner)];
        let split = super::super::split_block_in_inline(&[Node::Element(outer)]);
        let block = split
            .iter()
            .find_map(|n| match n {
                Node::Element(e) if e.tag == "div" => Some(e),
                _ => None,
            })
            .unwrap();
        let mut own = BoxStyle::default();
        own.cb_node = 9;
        own.cb_depth = 3;
        restore_containing_block(block, &mut own);
        assert_eq!((own.cb_node, own.cb_depth, own.cb_ancestor), (12, 5, true));
    }
}
