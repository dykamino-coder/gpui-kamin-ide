//! Анонимные объекты таблицы (CSS 2.1 §17.2.1): прогоны бесхозных табличных ролей в
//! не-табличном родителе заворачиваются в одну анонимную таблицу (в строчном — в
//! `inline-table`) на стороне `dom::table_boxes` (`wrap_anon_tables`, `inline_anon_tables`,
//! `anon_element`; первый и последний реэкспортированы ниже для layout). Здесь — починка
//! детей таблицы и рядов (`fixup`), логический сбор рядов (`rows`), проба «красится ли
//! коробка» и признак HTML-ячейки.

use crate::dom::Element;
use crate::style::computed::Computed;
mod fixup;
#[cfg(test)]
pub(super) use fixup::fixup_row_children;
pub(crate) use fixup::fixup_table_children;

mod rows;
pub(super) use rows::{RowCarry, TableRow, collect_rows, row_collapsed};

/// Красится ли коробка (фон или рамка) — такой блок в бюджете строк
/// прячется целиком, если точка среза попала внутрь него.
pub(crate) fn has_box_style_probe(c: &Computed) -> bool {
    c.background.is_some()
        || c.bg_image.is_some()
        || c.gradient_raw.is_some()
        || c.border_visible.contains(&Some(true))
}

pub(crate) use crate::dom::table_boxes::{anon_element, wrap_anon_tables};

/// An HTML `td`/`th` element, whose UA style inherits the row's
/// `vertical-align` (HTML §15.3.9); anonymous cells keep the initial value.
pub(super) fn html_cell(cell: &Element) -> bool {
    matches!(cell.tag.as_str(), "td" | "th") && !cell.marks.anonymous_cell
}
