//! Ребёнок многоколоночника с сеткой (`flow::blocks::grid_tree`): высоту, параллельный поток
//! и точки разрыва даёт раскладка копии в колонку (`StackChild::measure`; ряды и элементы
//! разложенной сетки — `flow::blocks`), а не оценка дорожек по стилю (css-grid-1 §12:
//! фрагментация идёт после раскладки сетки).

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::line_shape::resolved_lengths;
use crate::layout::fragment::{Shape, ShapeCx};
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
        && crate::layout::fragment::flow::blocks::grid_tree(c, 4)
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
    match super::children::measured_margins(&c) {
        Some((mt, mb)) => {
            measured_kids.borrow_mut().push((c.node_id, w));
            Some((
                c,
                Shape {
                    margin_top: mt,
                    margin_bottom: mb,
                    ..Shape::default()
                },
            ))
        }
        None => shape_full(&c, 4, ShapeCx::COLUMNS, vcx).map(|h| (c.clone(), h)),
    }
}
