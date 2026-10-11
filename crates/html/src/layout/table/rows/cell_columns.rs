//! Многоколоночная ячейка таблицы: содержимое ячейки режется на колонки.
//!
//! css-multicol-1 §2: многоколоночным контейнером становится любой БЛОЧНЫЙ
//! контейнер с `column-count`/`column-width`, а ячейка таблицы — блочный
//! контейнер (CSS 2.1 §9.2.1, §17.4). Blink так и раскладывает ячейку:
//! `LayoutTableCell` с колонками получает `LayoutMultiColumnFlowThread`
//! (`layout_block_flow.cc` `CreateOrDestroyMultiColumnFlowThreadIfNeeded`:
//! исключены только поля формы и т. п., но не ячейки). Сама ячейка у нас
//! строится путём таблицы, а колонки — путём обычного блока, поэтому
//! содержимое кладётся в анонимную блочную коробку с колоночными свойствами
//! ячейки: она занимает поле содержимого ячейки, как поток колонок
//! (`multicol-table-cell-001`, `multicol-table-cell-height-001/002`).

use crate::dom::{Element, Node};
use crate::layout::multicol::spanner::multicol_container;
use crate::style::computed::{Computed, Display};

/// Дети ячейки для раскладки: у многоколоночной ячейки — одна анонимная
/// блочная коробка-поток с её колоночными свойствами, иначе `None`.
pub(super) fn column_flow_children(cell: &Element) -> Option<Vec<Node>> {
    let s = &cell.style;
    if !multicol_container(s) || cell.children.is_empty() {
        return None;
    }
    let style = Computed {
        display: Some(Display::Block),
        column_count: s.column_count,
        column_width: s.column_width,
        column_height: s.column_height,
        column_wrap: s.column_wrap,
        column_gap: s.column_gap,
        column_fill_auto: s.column_fill_auto,
        column_rule_width: s.column_rule_width,
        column_rule_visible: s.column_rule_visible,
        column_rule_color: s.column_rule_color,
        column_rule_break: s.column_rule_break,
        column_rule_widths: s.column_rule_widths.clone(),
        column_rule_styles: s.column_rule_styles.clone(),
        column_rule_double: s.column_rule_double,
        column_rule_colors: s.column_rule_colors.clone(),
        column_rule_inset: s.column_rule_inset.clone(),
        column_rule_visibility: s.column_rule_visibility,
        ..Computed::default()
    };
    let flow = Element {
        list_item: None,
        node_id: cell.node_id ^ 0x0CE1_1C01_0000_0001,
        anim: None,
        tag: "div".to_string(),
        style,
        hover: None,
        first_letter: None,
        first_line: None,
        children: cell.children.clone(),
        attrs: Vec::new(),
        inline: false,
    };
    Some(vec![Node::Element(flow)])
}
