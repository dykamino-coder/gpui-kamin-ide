//! Мера сетки, flex, таблицы, обычной обёртки вложенных колонок и clamp-контейнера:
//! высоту и точки разрыва даёт раскладка копии (`StackChild::measure`).
//! Сумма размеров потомков не выражает блочный размер этих контекстов: у таблицы
//! высоты рядов, охваты и ячейки даёт только её раскладка (css-tables-3 §fragmentation).

use crate::dom::Element;
use crate::layout::cx::FrameCx;
use crate::layout::fragment::FlowMargins;
use crate::layout::fragment::line_shape::resolved_lengths;
use crate::style::computed::{BoxStyle, Display, Position};
use std::cell::RefCell;

fn multicol_wrapper(c: &Element) -> bool {
    // Flex/grid/table проходят свои проверки: вложенные колонки не отменяют
    // фрагментацию их форматирующего контекста (css-break-3 §2).
    // Сам многоколоночник требует специализированного пути с внешним
    // фрагментаинером; обычная обёртка сохраняет этот контекст в потомке.
    matches!(
        c.style.used().reset_box_layout.display,
        None | Some(Display::Block | Display::ListItem)
    ) && !crate::layout::multicol::spanner::multicol_container(&c.style.used())
        && crate::layout::multicol::spanner::multicol_inside(c, 4)
}

/// Блочный поточный ребёнок с сеткой, flex или таблицей в стопке с известной
/// шириной колонки;
/// `fits` — стопка допускает меру раскладкой (без рядов колонок).
pub(super) fn gate(c: &Element, fits: bool, line_col_w: Option<f32>) -> bool {
    fits && line_col_w.is_some()
        && !c.inline
        && c.style.used().reset_fragmentation.column_span != Some(true)
        && matches!(
            c.style.used().reset_positioned.position,
            None | Some(Position::Relative)
        )
        && c.style.used().reset_box_layout.float.unwrap_or(0) == 0
        && (crate::layout::fragment::flow::blocks::grid_tree(c)
            || crate::layout::fragment::flow::blocks::flex_tree(c)
            || crate::layout::fragment::flow::blocks::table_tree(c)
            || multicol_wrapper(c)
            || crate::layout::box_kind::clamp_host(c, &c.style.used()))
}

/// Flex, вложенные колонки и clamp-контейнер сами определяют свои строки; DOM-пагинация
/// теряет членство линий и точку среза.
pub(super) fn own_layout_gate(c: &Element, fits: bool, w: Option<f32>) -> bool {
    gate(c, fits, w)
        && (crate::layout::fragment::flow::blocks::flex_tree(c)
            || multicol_wrapper(c)
            || crate::layout::box_kind::clamp_host(c, &c.style.used()))
}

/// Запись ребёнка, чью высоту меряет раскладка копии; поля не в точках —
/// прежняя мера.
pub(super) fn measured(
    c: &Element,
    merged: &BoxStyle,
    vcx: FrameCx<'_>,
    w: f32,
    measured_kids: &RefCell<Vec<(u64, f32)>>,
) -> Option<(Element, FlowMargins)> {
    let c = resolved_lengths(c, merged, vcx);
    let c = crate::layout::block::margins::fragment_box(c, w, vcx.values);
    let (margin_top, margin_bottom) = crate::layout::block::margins::fragment_margins(&c)?;
    measured_kids.borrow_mut().push((c.node_id, w));
    Some((
        c,
        FlowMargins {
            margin_top,
            margin_bottom,
        },
    ))
}

#[cfg(test)]
mod tests;
