//! Деревья без побочных рекордеров допускают повтор числового баланса.

use crate::dom::{Element, Node};

/// Только текстовые блоки: остальные рекордеры должны работать в новом кадре.
pub(in crate::layout::multicol::deferred) fn simple_tree(host: &Element) -> bool {
    let mut pending = host
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
        .collect::<Vec<_>>();
    while let Some(e) = pending.pop() {
        let s = e.style.used();
        if !matches!(
            s.reset_box_layout.display,
            None | Some(crate::style::computed::Display::Block)
        ) || s.reset_box_layout.float.is_some()
            || s.reset_fragmentation.column_count.is_some()
            || s.reset_fragmentation.column_width.is_some()
            || e.marks.paginate
        {
            return false;
        }
        pending.extend(e.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        }));
    }
    true
}
