//! Сборка дерева: перенос слоя `::first-letter` в первый блочный потомок в потоке.
//!
//! CSS 2 §5.12.1–5.12.2 и css-pseudo-4 §first-letter-tree ставят фиктивный
//! строчный элемент внутрь потомка, который даёт первую форматированную строку;
//! относительные шрифтовые единицы остаются относительными к тому тексту.
//! Плавающие буквицы, `initial-letter` и пересечение с `::first-line` идут
//! своей раскладкой и сюда не попадают.

use crate::dom::Node;
use crate::dom::whitespace::blank_text;
use crate::layout::box_kind::{block_level_in_flow, out_of_flow};
use crate::layout::fragment::table_bands::table_box;
use crate::style::computed::{Computed, Display};

pub(super) fn route(mut nodes: Vec<Node>, parent: &Computed) -> Vec<Node> {
    let Some(first) = parent.first_letter_own.as_deref() else {
        return nodes;
    };
    // Floating initials and overlapping first-line layers use separate layout.
    if parent.first_letter.is_none()
        || first.float.is_some_and(|f| f != 0)
        || first.initial_letter.is_some()
        || parent.first_line.is_some()
        || parent.preserve_newlines == Some(true)
    {
        return nodes;
    }
    for node in &mut nodes {
        match node {
            Node::Text(t) if blank_text(t) => continue,
            Node::Element(e) if out_of_flow(&e.style.used()) || e.tag == "::marker" => continue,
            Node::Element(e)
                if block_level_in_flow(e)
                    && e.style.used().position.is_none()
                    && !table_box(e)
                    && matches!(
                        e.style.used().display,
                        None | Some(Display::Block | Display::ListItem)
                    )
                    && e.first_letter.is_none()
                    && e.first_line.is_none() =>
            {
                // CSS 2 sections 5.12.1-5.12.2 and CSS Pseudo 4
                // #first-letter-tree put this fictitious inline inside the
                // descendant. Keep relative fonts relative to that text.
                let layer = first.clone();
                e.style.used_mut().first_letter_own = Some(Box::new(layer.clone()));
                e.first_letter = Some(layer.into());
            }
            _ => {}
        }
        // A later sibling cannot supply the first formatted line.
        break;
    }
    nodes
}
