//! Ряды таблицы в порядке дорожек её сетки (`grid_rows`): тот же ремонт детей
//! (`fixup_table_children`, css-tables-3 §3), та же перестановка групп (`table_rows_fixed`,
//! CSS 2.1 §17.5.3) и тот же сбор рядов (`collect_rows`), что у раскладки `table()`, поэтому
//! ряд `i` стоит в дорожке `i` разложенной сетки. Фрагментация читает по ним высоты рядов из
//! раскладки (css-tables-3 §fragmentation), а не из оценки. Ключ таблицы (`table_key`) —
//! общий для записи раскладки и её читателя.

use super::anon::{RowCarry, collect_rows, fixup_table_children};
use super::setup::{row_span_in_group, rows_left_and_cols, table_rows_fixed};
use crate::document::state::ValueCx;
use crate::dom::{Element, Node};

/// Ряд таблицы в порядке дорожек сетки.
pub(crate) struct GridRow<'a> {
    pub(crate) row: &'a Element,
    /// Группа рядов; `None` — ряд прямо в таблице (анонимная группа).
    pub(crate) group: Option<&'a Element>,
    /// Ряд первой группы-шапки (`thead` или `table-header-group`).
    pub(crate) head: bool,
    /// Ряд первой группы-подвала.
    pub(crate) foot: bool,
    /// Рядов от этого до конца его группы включительно: предел `rowspan`.
    pub(crate) left: usize,
}

/// Ряды таблицы `e` в порядке дорожек её сетки.
pub(crate) fn grid_rows<R>(
    e: &Element,
    vcx: ValueCx<'_>,
    f: impl FnOnce(&[GridRow<'_>]) -> R,
) -> R {
    let fixed = table_rows_fixed(fixup_table_children(&e.children, vcx));
    let kind_of = |g: &Element| match g.tag.as_str() {
        "thead" => Some(0u8),
        "tbody" => Some(1),
        "tfoot" => Some(2),
        _ => g.style.cascaded().row_group_kind,
    };
    let first_of = |k: u8| {
        fixed.iter().find_map(|n| match n {
            Node::Element(g) if kind_of(g) == Some(k) => Some(g as *const Element),
            _ => None,
        })
    };
    let (head, foot) = (first_of(0), first_of(2));
    let mut rows = Vec::new();
    collect_rows(&fixed, Some(e), RowCarry::default(), &mut rows);
    let (left, _) = rows_left_and_cols(&rows);
    // Секция — группа рядов или ряд прямо в таблице со своей ролью.
    let in_section = |r: &super::anon::TableRow<'_>, s: Option<*const Element>| {
        s.is_some_and(|s| {
            r.carry.group.map(|g| g as *const Element) == Some(s)
                || std::ptr::eq(r.element as *const Element, s)
        })
    };
    let out: Vec<GridRow<'_>> = rows
        .iter()
        .zip(left)
        .map(|(r, left)| GridRow {
            row: r.element,
            group: r.carry.group,
            head: in_section(r, head),
            foot: in_section(r, foot),
            left,
        })
        .collect();
    f(&out)
}

/// Охват ячейки по рядам в пределах её группы (HTML table model).
pub(crate) fn cell_row_span(cell: &Element, left: usize) -> usize {
    row_span_in_group(cell, left)
}

/// Ключ таблицы для записи её раскладки: свой номер узла, у анонимной таблицы
/// (номер 0) — номер первого потомка с номером.
pub(crate) fn table_key(e: &Element) -> u64 {
    fn first(e: &Element) -> Option<u64> {
        e.children.iter().find_map(|n| match n {
            Node::Element(k) if k.node_id != 0 => Some(k.node_id),
            Node::Element(k) => first(k),
            _ => None,
        })
    }
    if e.node_id != 0 {
        e.node_id
    } else {
        first(e).unwrap_or(0)
    }
}
