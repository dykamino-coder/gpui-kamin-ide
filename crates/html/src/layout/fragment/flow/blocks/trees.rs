//! Поддеревья, чьи границы видит рекордер блочной стопки: блочные контейнеры, кладущие
//! детей стопкой (`stacks_blocks`), поточные поддеревья из стопок и монолитов
//! (`stacked_tree`), таблицы, чьи подписи и ячейки — обычные блоки (`table_tree`), и
//! строчные коробки с блоками внутри (`holds_blocks`, CSS 2.1 §9.2.1.1).

use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::clone::monolith_box;
use crate::layout::fragment::flow::box_children::children;
use crate::layout::fragment::table_bands::table_box;
use crate::style::computed::Display;

/// Блочный контейнер, чьи дети идут стопкой по блочной оси: блок и элемент
/// списка. Flex, сетка, таблица, многоколоночник и строчный набор кладут
/// детей иначе — их границы дают свои контексты (строки — `Paragraph`).
pub(super) fn stacks_blocks(e: &Element) -> bool {
    matches!(
        e.style.used().display,
        None | Some(Display::Block) | Some(Display::ListItem)
    ) && e.style.used().webkit_box != Some(true)
        && !crate::layout::multicol::spanner::multicol_container(&e.style.used())
        && !table_box(e)
        && !matches!(e.tag.as_str(), "tr" | "thead" | "tbody" | "tfoot")
}

/// Обычные блоки без контекстов с отдельной раскладкой. В отличие от
/// legacy probe проверяется всё дерево, без глубины оценщика.
pub(crate) fn plain_tree(e: &Element) -> bool {
    stacks_blocks(e)
        && children(&e.children).all(|n| match n {
            Node::Element(k) => k.inline || plain_tree(k),
            _ => true,
        })
}

/// Все коробки, перед которыми может встать распорка, видит рекордер.
/// Таблица внутри обычной стопки сохраняет собственные ряды и ячейки;
/// plain_tree здесь неприменим, поскольку намеренно исключает таблицы.
pub(crate) fn stacked_tree(e: &Element) -> bool {
    stacks_blocks(e)
        && children(&e.children).all(|n| match n {
            Node::Element(k) if out_of_flow(&k.style.used()) || k.inline => {
                !holds_blocks(k) || stacked_tree(k)
            }
            Node::Element(k) => monolith_box(k) || stacked_tree(k) || table_tree(k),
            _ => true,
        })
}

/// Таблица горизонтального письма, всё поддерево которой — части таблицы
/// (группы, ряды, ячейки, подписи) и поддеревья обычных блоков
/// (`plain_tree`): её подписи, ряды и содержимое ячеек видит рекордер
/// (`table::TableBox`), а не оценка формы.
pub(crate) fn table_tree(e: &Element) -> bool {
    fn part(n: &Node) -> bool {
        match n {
            Node::Element(k) if k.inline => true,
            Node::Element(k)
                if matches!(
                    k.style.used().display,
                    Some(Display::TableRow)
                        | Some(Display::TableRowGroup)
                        | Some(Display::TableCell)
                ) || matches!(
                    k.tag.as_str(),
                    "tr" | "thead" | "tbody" | "tfoot" | "td" | "th"
                ) =>
            {
                children(&k.children).all(part)
            }
            Node::Element(k) => {
                stacked_tree(k) || table_tree(k) || super::grid_tree(k) || super::flex_tree(k)
            }
            _ => true,
        }
    }
    table_box(e) && !e.style.cascaded().wm().vertical() && children(&e.children).all(part)
}

/// Строчная коробка с поточным блоком внутри (на любой глубине строчных).
pub(super) fn holds_blocks(e: &Element) -> bool {
    children(&e.children).any(|n| match n {
        Node::Element(k) if out_of_flow(&k.style.used()) => false,
        Node::Element(k) if !k.inline => true,
        Node::Element(k) => k.style.used().display.is_none() && holds_blocks(k),
        _ => false,
    })
}
