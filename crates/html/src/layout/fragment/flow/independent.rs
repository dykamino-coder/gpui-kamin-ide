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
    let mut logical = super::logical::recorded_tree(c, rl)?;
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
mod tests {

    use super::*;
    use crate::layout::fragment::{ShapeCx, flow::blocks::BlockBox};

    fn authored(nodes: &[Node]) -> &Element {
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        find(nodes).expect("authored root")
    }

    #[test]
    fn vertical_rows_keep_their_recorded_kind_and_original_physical_axes() {
        for (mode, rl) in [("vertical-lr", false), ("vertical-rl", true)] {
            for body in [
                "<div style='display:grid;grid-template-rows:20px 30px'><div></div><div></div></div>",
                "<table><tr><td><div></div></td></tr><tr><td></td></tr></table>",
            ] {
                let tree = crate::dom::parse(
                    &format!(
                        "<div style='writing-mode:{mode};width:60px;height:100px'>{body}</div>"
                    ),
                    "",
                );
                let physical = authored(&tree);
                assert!(vertical_copy(physical, rl), "{mode}: {body}");
                let logical = logical_copy(physical, rl).unwrap();
                assert!(!logical.style.wm().vertical());
                assert!(physical.style.wm().vertical());
                assert_eq!(logical.style.height, physical.style.width);
                assert_eq!(logical.style.width, physical.style.height);
                let blocks = BlockBox::collect(&logical, ShapeCx::COLUMNS, tree.values());
                if body.starts_with("<table") {
                    assert_eq!(blocks.table_keys().len(), 1);
                } else {
                    let mut grids = Vec::new();
                    blocks.grids(&mut grids);
                    assert_eq!(grids.len(), 1);
                    assert_eq!(grids[0].1.len(), 2);
                }
            }
        }
    }

    #[test]
    fn orthogonal_descendant_requires_its_own_continuation() {
        let tree = crate::dom::parse(
            "<div style='writing-mode:vertical-rl'><div style='writing-mode:horizontal-tb'></div></div>",
            "",
        );
        assert!(!vertical_copy(authored(&tree), true));
    }

    #[test]
    fn rtl_metadata_projects_sides_without_enabling_the_legacy_line_estimator() {
        use crate::style::values::value::Len;
        for (mode, rl) in [("vertical-lr", false), ("vertical-rl", true)] {
            let tree = crate::dom::parse(
                &format!(
                    "<div style='writing-mode:{mode};direction:rtl;margin:1px 2px 3px 4px'><div></div></div>"
                ),
                "",
            );
            let mut physical = authored(&tree).clone();
            physical.style.border_visible = [Some(true), Some(true), Some(false), Some(false)];
            assert!(vertical_copy(&physical, rl));
            assert!(super::super::transpose_tree(&physical, rl).is_none());
            let logical = logical_copy(&physical, rl).unwrap();
            assert_eq!(
                logical.style.margin.top,
                Some(Len::Px(if rl { 2.0 } else { 4.0 }))
            );
            assert_eq!(
                logical.style.margin.bottom,
                Some(Len::Px(if rl { 4.0 } else { 2.0 }))
            );
            assert_eq!(logical.style.margin.left, Some(Len::Px(3.0)));
            assert_eq!(logical.style.margin.right, Some(Len::Px(1.0)));
            assert_eq!(
                logical.style.border_visible,
                [Some(rl), Some(true), Some(!rl), Some(false)]
            );
            assert!(physical.style.wm().vertical());
        }
    }
}
