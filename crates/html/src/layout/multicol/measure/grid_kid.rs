//! Ребёнок многоколоночника с сеткой (`flow::blocks::grid_tree`): высоту, параллельный поток
//! и точки разрыва даёт раскладка копии в колонку (`StackChild::measure`; ряды и элементы
//! разложенной сетки — `flow::blocks`), а не оценка дорожек по стилю (css-grid-1 §12:
//! фрагментация идёт после раскладки сетки).

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::Shape;
use crate::layout::fragment::line_shape::resolved_lengths;
use crate::style::computed::{BoxStyle, Position};
use std::cell::RefCell;

/// Блочный поточный ребёнок с сеткой в стопке с известной шириной колонки;
/// `fits` — стопка допускает меру раскладкой (без рядов колонок).
pub(super) fn gate(c: &Element, fits: bool, line_col_w: Option<f32>) -> bool {
    fits && line_col_w.is_some()
        && !c.inline
        && c.style.computed().column_span != Some(true)
        && matches!(c.style.computed().position, None | Some(Position::Relative))
        && c.style.computed().float.unwrap_or(0) == 0
        && (crate::layout::fragment::flow::blocks::grid_tree(c, 4)
            || crate::layout::fragment::flow::blocks::flex_tree(c, 4))
}

/// Flex с реальными линиями не передаётся DOM-пагинации: та не сохраняет членство.
pub(super) fn flex_gate(c: &Element, fits: bool, w: Option<f32>) -> bool {
    gate(c, fits, w) && crate::layout::fragment::flow::blocks::flex_tree(c, 4)
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
            &Computed::default(),
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
            measured(c, &Computed::default(), tree.values(), 120.0, &registry).expect("copy");
        assert_eq!((shape.margin_top, shape.margin_bottom), (12.0, 6.0));
        assert_eq!(shape.height, 0.0);
        assert_eq!(*registry.borrow(), vec![(c.node_id, 120.0)]);
    }
}
