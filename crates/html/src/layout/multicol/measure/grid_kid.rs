//! Ребёнок многоколоночника с сеткой (`flow::blocks::grid_tree`): высоту, параллельный поток
//! и точки разрыва даёт раскладка копии в колонку (`StackChild::measure`; ряды и элементы
//! разложенной сетки — `flow::blocks`), а не оценка дорожек по стилю (css-grid-1 §12:
//! фрагментация идёт после раскладки сетки).

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::Shape;
use crate::layout::fragment::line_shape::resolved_lengths;
use crate::style::computed::{Computed, Position};
use std::cell::RefCell;

/// Блочный поточный ребёнок с сеткой в стопке с известной шириной колонки;
/// `fits` — стопка допускает меру раскладкой (без рядов колонок).
pub(super) fn gate(c: &Element, fits: bool, line_col_w: Option<f32>) -> bool {
    fits && line_col_w.is_some()
        && !c.inline
        && c.style.column_span != Some(true)
        && matches!(c.style.position, None | Some(Position::Relative))
        && c.style.float.unwrap_or(0) == 0
        && (crate::layout::fragment::flow::blocks::grid_tree(c, 4)
            || crate::layout::fragment::flow::blocks::flex_tree(c, 4))
}

/// Запись ребёнка, чью высоту меряет раскладка копии; поля не в точках —
/// прежняя мера.
pub(super) fn measured(
    c: &Element,
    merged: &Computed,
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
    fn percent_margins_use_the_copy_inline_size() {
        let tree = crate::dom::parse(
            "<div style='display:grid;margin-top:10%;margin-bottom:5%'><div>x</div></div>",
            "",
        );
        let crate::dom::Node::Element(c) = &tree[0] else {
            panic!("grid")
        };
        let mut c = c;
        while c.tag != "div" {
            let crate::dom::Node::Element(child) = &c.children[0] else {
                panic!("grid ancestor")
            };
            c = child;
        }
        let registry = RefCell::default();
        let (_, shape) =
            measured(c, &Computed::default(), tree.values(), 120.0, &registry).expect("copy");
        assert_eq!((shape.margin_top, shape.margin_bottom), (12.0, 6.0));
        assert_eq!(shape.height, 0.0);
        assert_eq!(*registry.borrow(), vec![(c.node_id, 120.0)]);
    }
}
