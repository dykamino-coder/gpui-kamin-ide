//! Деревья копий, чьи коробки и строки целиком доступны рекордеру раскладки.

use crate::dom::{Element, Node};
use crate::layout::fragment::push::measured_tree;
use crate::style::computed::Position;

/// У копии нет отдельного плана фрагментов или параллельного потока.
/// Отдельные планы clone, OOF и nested требуют продолжений F2-1/F2-2/F2-3.
pub(crate) fn independent_copy(c: &Element) -> bool {
    !c.inline && measured_tree(c) && !separate(c, false)
}

/// Обычные блоки одного вертикального потока; ортогональные продолжения — F2-3.
pub(crate) fn vertical_copy(c: &Element, rl: bool) -> bool {
    !c.inline
        && crate::layout::fragment::probe::plain_block_tree(c, 4)
        && crate::layout::fragment::flow::blocks::stacked_tree(c, 4)
        && crate::layout::fragment::line_shape::transpose_tree(c, rl).is_some()
        && !separate(c, true)
}

fn separate(e: &Element, vertical: bool) -> bool {
    e.style.bdb_clone
        || e.style.text_box_trim_start
        || e.style.text_box_trim_end
        || e.marks.host_trim_start
        || e.marks.host_trim_end
        || (!vertical && e.style.vertical_decl() == Some(true))
        || e.style.float.is_some_and(|f| f != 0)
        || matches!(e.style.position, Some(Position::Absolute | Position::Fixed))
        || crate::layout::multicol::spanner::multicol_container(&e.style)
        || e.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if separate(k, vertical)))
}
