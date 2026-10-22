//! Поддеревья, чьи границы видит рекордер блочной стопки: блочные контейнеры, кладущие
//! детей стопкой (`stacks_blocks`), поточные поддеревья из стопок и монолитов
//! (`stacked_tree`), таблицы, чьи подписи и ячейки — обычные блоки (`table_tree`), и
//! строчные коробки с блоками внутри (`holds_blocks`, CSS 2.1 §9.2.1.1).

use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::clone::monolith_box;
use crate::layout::fragment::probe::plain_block_tree;
use crate::layout::fragment::table_bands::table_box;
use crate::style::computed::Display;

/// Блочный контейнер, чьи дети идут стопкой по блочной оси: блок и элемент
/// списка. Flex, сетка, таблица, многоколоночник и строчный набор кладут
/// детей иначе — их границы дают свои контексты (строки — `Paragraph`).
pub(super) fn stacks_blocks(e: &Element) -> bool {
    matches!(
        e.style.display,
        None | Some(Display::Block) | Some(Display::ListItem)
    ) && e.style.webkit_box != Some(true)
        && !crate::layout::multicol::spanner::multicol_container(&e.style)
        && !table_box(e)
        && !matches!(e.tag.as_str(), "tr" | "thead" | "tbody" | "tfoot")
}

/// Всё поточное блочное поддерево `e` до глубины `depth` — блочные стопки и
/// монолиты: каждую коробку, перед которой может встать распорка, рекордер
/// стопки видит (таблица, flex и сетка кладут детей иначе).
pub(crate) fn stacked_tree(e: &Element, depth: u8) -> bool {
    stacks_blocks(e)
        && (depth == 0
            || e.children.iter().all(|n| match n {
                Node::Element(k) if out_of_flow(&k.style) || k.inline => {
                    !holds_blocks(k) || stacked_tree(k, depth - 1)
                }
                Node::Element(k) => {
                    monolith_box(k) || stacked_tree(k, depth - 1) || table_tree(k, depth - 1)
                }
                _ => true,
            }))
}

/// Таблица горизонтального письма, всё поддерево которой — части таблицы
/// (группы, ряды, ячейки, подписи) и поддеревья обычных блоков
/// (`plain_block_tree`): её подписи, ряды и содержимое ячеек видит рекордер
/// (`table::TableBox`), а не оценка формы.
pub(crate) fn table_tree(e: &Element, depth: u8) -> bool {
    fn part(n: &Node, depth: u8) -> bool {
        match n {
            Node::Element(k) if k.inline => true,
            Node::Element(k)
                if matches!(
                    k.style.display,
                    Some(Display::TableRow)
                        | Some(Display::TableRowGroup)
                        | Some(Display::TableCell)
                ) =>
            {
                depth > 0 && k.children.iter().all(|n| part(n, depth - 1))
            }
            Node::Element(k) => {
                plain_block_tree(k, depth) || table_tree(k, depth) || super::grid_tree(k, depth)
            }
            _ => true,
        }
    }
    table_box(e)
        && !e.style.wm().vertical()
        && depth > 0
        && e.children.iter().all(|n| part(n, depth - 1))
}

/// Строчная коробка с поточным блоком внутри (на любой глубине строчных).
pub(super) fn holds_blocks(e: &Element) -> bool {
    e.children.iter().any(|n| match n {
        Node::Element(k) if out_of_flow(&k.style) => false,
        Node::Element(k) if !k.inline => true,
        Node::Element(k) => k.style.display.is_none() && holds_blocks(k),
        _ => false,
    })
}
