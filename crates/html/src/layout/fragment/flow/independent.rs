//! Деревья копий, чьи коробки и строки целиком доступны рекордеру раскладки.

use crate::dom::{Element, Node};
use crate::layout::fragment::push::measured_tree;
use crate::style::computed::Position;

/// У копии нет отдельного плана фрагментов или параллельного потока.
/// Отдельные планы clone, OOF и nested требуют продолжений F2-1/F2-2/F2-3.
pub(crate) fn independent_copy(c: &Element) -> bool {
    !c.inline && measured_tree(c) && !separate(c, false)
}

/// Блоки, сетки и таблицы одного вертикального потока.
/// Ортогональные продолжения требуют ADR P-19 F2-3.
pub(crate) fn vertical_copy(c: &Element, rl: bool) -> bool {
    let Some(logical) = logical_copy(c, rl) else {
        return false;
    };
    use super::blocks::{grid_tree, stacked_tree, table_tree};
    !c.inline
        && !c.marks.paginate
        && c.style.column_span != Some(true)
        && crate::layout::fragment::clone::clone_dec(c).is_none()
        && !separate(c, true)
        && ((crate::layout::fragment::probe::plain_block_tree(&logical, 4)
            && stacked_tree(&logical, 4))
            || table_tree(&logical, 4)
            || grid_tree(&logical, 4))
}

/// Только метаданные рекордера: физическое дерево рисуется без поворота.
pub(crate) fn logical_copy(c: &Element, rl: bool) -> Option<Element> {
    fn horizontal(e: &mut Element) {
        use crate::style::computed::writing_mode::WritingModeKw::HorizontalTb;
        e.style.writing_mode = Some(HorizontalTb);
        e.style.flat_mode = Some(HorizontalTb);
        for n in &mut e.children {
            if let Node::Element(k) = n {
                horizontal(k);
            }
        }
    }
    let mut logical = crate::layout::fragment::line_shape::transpose_tree(c, rl)?;
    horizontal(&mut logical);
    Some(logical)
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

#[cfg(test)]
mod tests;
