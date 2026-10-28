//! Какие поддеревья с сеткой фрагментирует раскладка копии (`grid_tree`): всю геометрию
//! разрывов и рост которых дают `flow::blocks` и `push::Regrow`.

use super::grid_container;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::clone::monolith_blockified;
use crate::layout::fragment::flow::box_children::children;
use crate::style::computed::Display;

/// Поддерево блоков и сеток с хотя бы одной сеткой, всю геометрию разрывов
/// и рост которого даёт раскладка копии (`flow::blocks`, `push::Regrow`):
/// блочные стопки, сетки, чьи элементы — такие же поддеревья, flex, таблицы
/// или монолиты. Строки элемента сетки — его собственный параллельный поток
/// (адреса роста строк, `Watched::item_flows`); внепоточные потомки
/// продолжаются своим потоком, а их статическое положение даёт раскладка
/// копии. Не входят: многоколоночники, коробки со своим планом фрагментов
/// (`box-decoration-break: clone`) и обрезающие переполнение.
pub(crate) fn grid_tree(e: &Element) -> bool {
    measured_tree(e, false) && holds_grid(e)
}

/// Поддерево сетки, чьи параллельные потоки — только элементы сеток: каждый элемент
/// продолжается со своего расхода (`multicol::column_regrow::items`), и монолит одного
/// элемента может переполнить фрагментаинер, не растягивая соседей. Ряды flex и таблицы
/// такого расхода не ведут.
pub(crate) fn grid_items_resume(e: &Element) -> bool {
    fn serial(e: &Element) -> bool {
        !super::flex_container(e)
            && !crate::layout::fragment::table_bands::table_box(e)
            && children(&e.children).all(|n| match n {
                Node::Element(k) if out_of_flow(&k.style.used()) => true,
                Node::Element(k) => serial(k),
                _ => true,
            })
    }
    grid_tree(e) && serial(e)
}

fn holds_grid(e: &Element) -> bool {
    grid_container(e)
        || children(&e.children).any(|n| {
            matches!(n, Node::Element(k)
                    if !k.inline && !out_of_flow(&k.style.used()) && holds_grid(k))
        })
}

/// `item` — элемент сетки: его `inline-block` блокифицирован.
fn measured_tree(e: &Element, item: bool) -> bool {
    if crate::layout::multicol::spanner::multicol_container(&e.style.used())
        || e.style.used().webkit_box == Some(true)
        || crate::layout::fragment::clone::clone_dec(e).is_some()
        || !crate::layout::page::paged::visible_overflow(&e.style.used())
    {
        return false;
    }
    if crate::layout::fragment::table_bands::table_box(e) {
        return super::table_tree(e);
    }
    if grid_container(e) {
        return children(&e.children).all(|n| match n {
            Node::Element(k) if out_of_flow(&k.style.used()) => true,
            Node::Element(k) if matches!(k.style.used().display, Some(Display::None)) => true,
            Node::Element(k) => {
                monolith_blockified(k) || measured_tree(k, true) || super::flex_tree(k)
            }
            n => is_blank(n),
        });
    }
    let block = matches!(
        e.style.used().display,
        None | Some(Display::Block) | Some(Display::ListItem)
    ) || (item && e.style.used().display == Some(Display::InlineBlock));
    block
        && children(&e.children).all(|n| match n {
            Node::Element(k) if out_of_flow(&k.style.used()) => true,
            // Атомарная строчная коробка стоит в строке своего потока.
            Node::Element(k) if crate::layout::fragment::flex_lines::inline_display(k) => true,
            // Строчная коробка: блоки внутри неё — соседи потока
            // (CSS 2.1 §9.2.1.1), остальное — строки.
            Node::Element(k) if k.inline => {
                !children(&k.children).any(|x| matches!(x, Node::Element(b) if !b.inline))
                    || measured_tree(k, false)
            }
            Node::Element(k) => {
                crate::layout::fragment::clone::monolith_box(k)
                    || measured_tree(k, false)
                    || super::flex_tree(k)
            }
            _ => true,
        })
}
