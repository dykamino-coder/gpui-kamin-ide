//! Сборка дерева: перенос слоя `::first-line` в первый блочный потомок в потоке.
//!
//! CSS 2 §5.12.1: «The "first formatted line" of an element may occur inside
//! a block-level descendant in the same flow (i.e., a block-level descendant
//! that is not out-of-flow due to floating or positioning)»; фиктивная
//! последовательность тегов `::first-line` ставится внутрь этого потомка
//! (css-pseudo-4 §first-line-inheritance). Переезжают только объявления
//! псевдоэлемента: всё, чего он не задаёт, остаётся собственным стилем текста,
//! как и в `first_letter_descendants`.

use crate::dom::Node;
use crate::layout::box_kind::{block_level_in_flow, out_of_flow};
use crate::layout::fragment::table_bands::table_box;
use crate::style::computed::{Computed, Display};
use crate::text::text_box::blank_text;

pub(crate) fn route(mut nodes: Vec<Node>, parent: &Computed) -> Vec<Node> {
    let Some(first) = parent.first_line_own.as_deref() else {
        return nodes;
    };
    for node in &mut nodes {
        match node {
            Node::Text(t) if blank_text(t) => continue,
            Node::Element(e) if out_of_flow(&e.style) => continue,
            Node::Element(e)
                if block_level_in_flow(e)
                    && !table_box(e)
                    && matches!(
                        e.style.display,
                        None | Some(Display::Block | Display::ListItem)
                    )
                    && e.first_line.is_none() =>
            {
                e.style.first_line_own = Some(Box::new(first.clone()));
                e.first_line = Some(first.clone().into());
            }
            _ => {}
        }
        // A later sibling cannot supply the first formatted line.
        break;
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Element;
    use crate::style::cascade::inherit::inherit;
    use crate::style::computed::Position;
    use crate::style::values::value::Len;

    #[test]
    fn nested_first_line_skips_positioned_children_at_each_level() {
        let doc = crate::document::state::DocState::new(Default::default());
        let first = Computed {
            font_size: Some(Len::Px(32.)),
            ..Default::default()
        };
        let parent = Computed {
            first_line_own: Some(Box::new(first)),
            ..Default::default()
        };
        let positioned = Element::synthetic(
            "div",
            Computed {
                position: Some(Position::Absolute),
                ..Default::default()
            },
            Default::default(),
            vec![],
        );
        let block = Element::synthetic("div", Computed::default(), Default::default(), vec![]);
        let mut inherited = parent;
        for _ in 0..3 {
            let nodes = route(
                vec![
                    Node::Element(positioned.clone()),
                    Node::Element(block.clone()),
                ],
                &inherited,
            );
            let Node::Element(oof) = &nodes[0] else {
                panic!("positioned box")
            };
            assert!(oof.first_line.is_none());
            let Node::Element(child) = &nodes[1] else {
                panic!("in-flow block")
            };
            let mut merged = inherit(&inherited, &child.style, doc.values());
            crate::dom::pseudo_line_layers::install(child, &mut merged, doc.values());
            assert_eq!(
                merged.first_line.as_ref().unwrap().font_size,
                Some(Len::Px(32.))
            );
            assert_ne!(merged.font_size, Some(Len::Px(32.)));
            inherited = merged;
        }
    }
}
