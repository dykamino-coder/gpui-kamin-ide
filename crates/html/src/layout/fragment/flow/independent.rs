//! Деревья копий, чьи коробки и строки целиком доступны рекордеру раскладки.

use crate::dom::{Element, Node};
use crate::layout::fragment::push::measured_tree;
use crate::style::computed::{Display, Position};
mod continuation;
use continuation::continuation;
pub(crate) use continuation::parallel_overflow;

/// У копии нет отдельного плана фрагментов или параллельного потока.
/// Отдельные планы clone, OOF и nested требуют продолжений F2-1/F2-2/F2-3.
pub(crate) fn independent_copy(c: &Element) -> bool {
    !c.inline && measured_tree(c) && !separate(c, false) && !continuation(c)
}

/// Блоки, сетки и таблицы одного вертикального потока.
/// Ортогональные продолжения требуют ADR P-19 F2-3.
pub(crate) fn vertical_copy(c: &Element, rl: bool) -> bool {
    let Some(logical) = logical_copy(c, rl) else {
        return false;
    };
    use super::blocks::{flex_tree, grid_tree, stacked_tree, table_tree};
    !c.inline
        && !c.marks.paginate
        && c.style.used().column_span != Some(true)
        && crate::layout::fragment::clone::clone_dec(c).is_none()
        && (super::empty_floats(&logical)
            || (!separate(c, true)
                && !continuation::inline_continuation(c)
                && (stacked_tree(&logical)
                    || table_tree(&logical)
                    || grid_tree(&logical)
                    || flex_tree(&logical))))
}

/// Только метаданные рекордера: физическое дерево рисуется без поворота.
pub(crate) fn logical_copy(c: &Element, rl: bool) -> Option<Element> {
    fn horizontal(e: &mut Element) {
        use crate::style::computed::writing_mode::WritingModeKw::HorizontalTb;
        e.style.used_mut().writing_mode = Some(HorizontalTb);
        e.style.used_mut().flat_mode = Some(HorizontalTb);
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
    if e.style.used().display == Some(Display::None) {
        return false;
    }
    (e.style.used().display != Some(Display::Contents)
        && (e.style.used().bdb_clone
            || e.style.used().text_box_trim_start
            || e.style.used().text_box_trim_end
            || e.marks.host_trim_start
            || e.marks.host_trim_end
            || (!vertical && e.style.used().vertical_decl() == Some(true))
            || e.style.used().float.is_some_and(|f| f != 0)
            || matches!(
                e.style.used().position,
                Some(Position::Absolute | Position::Fixed)
            )
            || crate::layout::multicol::spanner::multicol_container(&e.style.used())))
        || e.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if separate(k, vertical)))
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::layout::fragment::{ShapeCx, flow::blocks::BlockBox};

    #[test]
    fn ordinary_horizontal_flex_uses_native_copy_geometry() {
        for css in [
            "",
            "flex-direction:column",
            "flex-direction:row-reverse;flex-wrap:wrap-reverse",
        ] {
            let tree = crate::dom::parse(
                &format!(
                    "<div style='display:flex;{css}'><div style='height:20px'></div><div style='height:30px'></div></div>"
                ),
                "",
            );
            assert!(independent_copy(authored(&tree)), "{css}");
        }
    }

    #[test]
    fn independent_copy_requires_a_single_consumed_flow() {
        for body in [
            "<div style='height:50px'><div style='height:150px'></div></div>",
            "<div style='max-height:0'><div style='height:150px'></div></div>",
            "<div style='min-height:200px'><div style='height:160px'></div></div>",
        ] {
            let tree = crate::dom::parse(body, "");
            assert!(!independent_copy(authored(&tree)), "{body}");
        }
        for body in [
            "<div style='display:flex'><div></div></div>",
            "<div><div style='display:flex;flex-direction:column'></div></div>",
            "<div><div style='height:20px'></div><div style='height:30px'></div></div>",
            "<div style='height:50px'></div>",
            "<div style='height:50px;overflow:hidden'><div style='height:150px'></div></div>",
            "<div style='min-height:50px;overflow:hidden'><div style='height:20px'></div></div>",
        ] {
            let tree = crate::dom::parse(body, "");
            assert!(independent_copy(authored(&tree)), "{body}");
        }
    }

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
                assert!(!logical.style.used().wm().vertical());
                assert!(physical.style.used().wm().vertical());
                assert_eq!(logical.style.used().height, physical.style.used().width);
                assert_eq!(logical.style.used().width, physical.style.used().height);
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
            physical.style.used_mut().border_visible =
                [Some(true), Some(true), Some(false), Some(false)];
            assert!(vertical_copy(&physical, rl));
            assert!(super::super::transpose_tree(&physical, rl).is_none());
            let logical = logical_copy(&physical, rl).unwrap();
            assert_eq!(
                logical.style.used().margin.top,
                Some(Len::Px(if rl { 2.0 } else { 4.0 }))
            );
            assert_eq!(
                logical.style.used().margin.bottom,
                Some(Len::Px(if rl { 4.0 } else { 2.0 }))
            );
            assert_eq!(logical.style.used().margin.left, Some(Len::Px(3.0)));
            assert_eq!(logical.style.used().margin.right, Some(Len::Px(1.0)));
            assert_eq!(
                logical.style.used().border_visible,
                [Some(rl), Some(true), Some(!rl), Some(false)]
            );
            assert!(physical.style.used().wm().vertical());
        }
    }
}
