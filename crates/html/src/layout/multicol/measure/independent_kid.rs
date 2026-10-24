//! Мера сетки, flex и обычной обёртки вложенных колонок:
//! высоту и точки разрыва даёт раскладка копии (`StackChild::measure`).
//! Сумма размеров потомков не выражает блочный размер этих контекстов.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::Shape;
use crate::layout::fragment::line_shape::resolved_lengths;
use crate::style::computed::{BoxStyle, Position};
use std::cell::RefCell;

fn multicol_wrapper(c: &Element) -> bool {
    // Сам многоколоночник требует специализированного пути с внешним
    // фрагментаинером; обычная обёртка сохраняет этот контекст в потомке.
    !crate::layout::multicol::spanner::multicol_container(&c.style.used())
        && crate::layout::multicol::spanner::multicol_inside(c, 4)
}

/// Блочный поточный ребёнок с сеткой в стопке с известной шириной колонки;
/// `fits` — стопка допускает меру раскладкой (без рядов колонок).
pub(super) fn gate(c: &Element, fits: bool, line_col_w: Option<f32>) -> bool {
    fits && line_col_w.is_some()
        && !c.inline
        && c.style.used().column_span != Some(true)
        && matches!(c.style.used().position, None | Some(Position::Relative))
        && c.style.used().float.unwrap_or(0) == 0
        && (crate::layout::fragment::flow::blocks::grid_tree(c)
            || crate::layout::fragment::flow::blocks::flex_tree(c)
            || multicol_wrapper(c))
}

/// Flex и вложенные колонки сами определяют линии; DOM-пагинация теряет их членство.
pub(super) fn own_layout_gate(c: &Element, fits: bool, w: Option<f32>) -> bool {
    gate(c, fits, w) && (crate::layout::fragment::flow::blocks::flex_tree(c) || multicol_wrapper(c))
}

/// Запись ребёнка, чью высоту меряет раскладка копии; поля не в точках —
/// прежняя мера.
pub(super) fn measured(
    c: &Element,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
    w: f32,
    measured_kids: &RefCell<Vec<(u64, f32)>>,
) -> Option<(Element, Shape)> {
    let c = resolved_lengths(c, merged, vcx);
    let c = crate::layout::block::margins::fragment_box(c, w, vcx);
    let (margin_top, margin_bottom) = crate::layout::block::margins::fragment_margins(&c)?;
    measured_kids.borrow_mut().push((c.node_id, w));
    Some((
        c,
        Shape {
            margin_top,
            margin_bottom,
            ..Shape::default()
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_grid_uses_measured_copy_at_known_and_deferred_widths() {
        use crate::dom::Node;
        use crate::layout::fragment::types::StackAxis;
        use crate::layout::multicol::deferred::Recipe;
        use crate::layout::options::RenderOpts;
        let tree = crate::dom::parse(
            "<div><div style='display:grid;grid-template-rows:20px 30px'></div></div>",
            "",
        );
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        let opts = RenderOpts {
            text: Default::default(),
            viewport: (800.0, 600.0),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        };
        let host = find(&tree).unwrap();
        let recipe = Recipe::new(
            host,
            &BoxStyle::default(),
            &opts,
            2,
            0.0,
            Some(100.0),
            StackAxis::Horizontal,
            None,
        )
        .unwrap();
        for width in [100.0, 120.0] {
            let kids = recipe.children(width);
            assert_eq!(kids.len(), 1);
            assert_eq!(kids[0].measure, Some(width));
            let Node::Element(grid) = &host.children[0] else {
                panic!("grid")
            };
            assert!(gate(grid, true, Some(width)));
            assert!(kids[0].watched.as_ref().is_some_and(|w| w.has_blocks()));
        }
        #[cfg(feature = "wpt-harness")]
        assert!(
            opts.frag
                .probe
                .take()
                .values()
                .all(|p| p.estimate_calls == 0)
        );
    }

    #[test]
    fn percent_margins_use_the_copy_inline_size() {
        let tree = crate::dom::parse(
            "<div style='display:grid;margin-top:10%;margin-bottom:5%'><div>x</div></div>",
            "",
        );
        fn find(nodes: &[crate::dom::Node]) -> Option<&Element> {
            nodes.iter().find_map(|n| match n {
                crate::dom::Node::Element(e) if e.tag == "div" => Some(e),
                crate::dom::Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        let c = find(&tree).expect("authored grid");
        let registry = RefCell::default();
        let (_, shape) =
            measured(c, &BoxStyle::default(), tree.values(), 120.0, &registry).expect("copy");
        assert_eq!((shape.margin_top, shape.margin_bottom), (12.0, 6.0));
        assert_eq!(shape.height, 0.0);
        assert_eq!(*registry.borrow(), vec![(c.node_id, 120.0)]);
    }
}
