//! Какие поддеревья с сеткой фрагментирует раскладка копии (`grid_tree`): всю геометрию
//! разрывов и рост которых дают `flow::blocks` и `push::Regrow`.

use super::grid_container;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::clone::monolith_blockified;
use crate::style::computed::Display;

/// Поддерево блоков и сеток с хотя бы одной сеткой, всю геометрию разрывов
/// и рост которого даёт раскладка копии (`flow::blocks`, `push::Regrow`):
/// блочные стопки, сетки, чьи элементы — такие же поддеревья или монолиты.
/// Не входят: внепоточные потомки (их продолжение — отдельный поток
/// фрагментации), многоколоночники, flex-контейнеры, коробки со
/// своим планом фрагментов (`box-decoration-break: clone`) или обрезающие
/// переполнение, и строчное содержимое внутри элементов сетки: строку,
/// перенесённую в следующий фрагментаинер, распорка в дереве копии не
/// сдвигает, а без сдвига фрагмент элемента не дотягивается до конца
/// фрагментаинера (css-break-3 §box-splitting) — это делают края строк и
/// фрагменты-виды (F1-2, F2).
pub(crate) fn grid_tree(e: &Element, depth: u8) -> bool {
    measured_tree(e, depth, false, true) && holds_grid(e, depth)
}

fn holds_grid(e: &Element, depth: u8) -> bool {
    grid_container(e)
        || (depth > 0
            && e.children.iter().any(|n| {
                matches!(n, Node::Element(k)
                    if !k.inline && !out_of_flow(&k.style) && holds_grid(k, depth - 1))
            }))
}

/// `item` — элемент сетки: его `inline-block` блокифицирован; `lines` —
/// строчное содержимое допустимо (вне элементов сетки).
fn measured_tree(e: &Element, depth: u8, item: bool, lines: bool) -> bool {
    if crate::layout::multicol::spanner::multicol_container(&e.style)
        || e.style.webkit_box == Some(true)
        || crate::layout::fragment::clone::clone_dec(e).is_some()
        || !crate::layout::page::paged::visible_overflow(&e.style)
    {
        return false;
    }
    if crate::layout::fragment::table_bands::table_box(e) {
        // Внутри элемента сетки строки таблицы — отдельные параллельные
        // потоки (F2-4/F2-5); вне него таблицу уже читает TableBox.
        return lines && super::table_tree(e, depth);
    }
    if grid_container(e) {
        // F1-6: у пустой сетки дорожки рассчитаны native layout, но API
        // parent_grid_tracks требует ребёнка. Нужен reader по LayoutId
        // самой сетки в vendor/gpui (сейчас вне scope); это НЕ F2-блокер.
        let items = e.children.iter().any(|n| {
            matches!(n, Node::Element(k)
                if !out_of_flow(&k.style) && !matches!(k.style.display, Some(Display::None)))
        });
        return depth > 0
            && items
            && e.children.iter().all(|n| match n {
                Node::Element(k) if out_of_flow(&k.style) => false,
                Node::Element(k) if matches!(k.style.display, Some(Display::None)) => true,
                Node::Element(k) => {
                    monolith_blockified(k) || measured_tree(k, depth - 1, true, false)
                }
                n => is_blank(n),
            });
    }
    let block = matches!(
        e.style.display,
        None | Some(Display::Block) | Some(Display::ListItem)
    ) || (item && e.style.display == Some(Display::InlineBlock));
    block
        && (depth == 0
            || e.children.iter().all(|n| match n {
                Node::Element(k) if out_of_flow(&k.style) => false,
                Node::Element(_) | Node::Text(_) if !lines && !is_blank(n) && n_inline(n) => false,
                // Строчная коробка: блоки внутри неё — соседи потока
                // (CSS 2.1 §9.2.1.1), остальное — строки.
                Node::Element(k) if k.inline => {
                    !k.children
                        .iter()
                        .any(|x| matches!(x, Node::Element(b) if !b.inline))
                        || measured_tree(k, depth - 1, false, lines)
                }
                Node::Element(k) => {
                    crate::layout::fragment::clone::monolith_box(k)
                        || measured_tree(k, depth - 1, false, lines)
                }
                _ => true,
            }))
}

/// Строчный узел: текст или строчный элемент.
fn n_inline(n: &Node) -> bool {
    match n {
        Node::Element(k) => k.inline || crate::layout::box_kind::inline_level(k),
        _ => true,
    }
}
