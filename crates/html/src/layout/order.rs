//! Решения о порядке paint позиционированных боксов с `z-index: auto | 0`.
//!
//! CSS 2.1 Appendix E, шаг 8, рисует такие боксы после in-flow содержимого их
//! stacking context, в порядке дерева. Построение переносит бокс в коллектор
//! шага 8, только если ни один более поздний позиционированный бокс не остаётся в
//! первом проходе; этот модуль один раз обходит документ (`unkeyed_positions`) и
//! отвечает на этот вопрос (`paint_last_ok`). GPUI-обёртки остаются у построителей.

use crate::dom::{Element, Node};
use crate::layout::box_kind::block_level_in_flow;
use crate::layout::cx::LayoutCx;
use crate::layout::positioned::predicates::positioned_later;
use crate::style::computed::Display;

/// Позиции узлов в прямом обходе документа и позиция ПОСЛЕДНЕГО
/// позиционированного, который ключа краски не получает (см.
/// `unkeyed_positions`).
pub(crate) type Unkeyed = (std::collections::HashMap<u64, usize>, Option<usize>);

/// Позиционированный с `z-index: auto | 0`, которого сборщик не оборачивает
/// в `PaintLast`: части таблицы (их строит табличный сборщик),
/// `relative`/`sticky` строчного уровня и строчный абсолют — те идут в
/// абзац. Такие красятся первым проходом там, где стоят.
fn unkeyed_positioned(e: &Element) -> bool {
    if e.style.used().z_index.unwrap_or(0) != 0 {
        return false;
    }
    let table_part = matches!(
        e.style.used().display,
        Some(Display::TableRow) | Some(Display::TableCell) | Some(Display::TableRowGroup)
    ) || (e.style.used().display.is_none()
        && matches!(
            e.tag.as_str(),
            "tr" | "td" | "th" | "tbody" | "thead" | "tfoot" | "caption" | "col" | "colgroup"
        ));
    match e.style.used().position {
        Some(crate::style::computed::Position::Relative)
        | Some(crate::style::computed::Position::Sticky) => table_part || !block_level_in_flow(e),
        Some(crate::style::computed::Position::Absolute) => {
            table_part || (e.style.used().display.is_none() && e.inline)
        }
        _ => false,
    }
}

/// Прямой обход документа: конец поддерева каждого узла (позиция за его
/// последним потомком) и позиция последнего позиционированного без ключа
/// краски.
pub(crate) fn unkeyed_positions(
    nodes: &[Node],
) -> (std::collections::HashMap<u64, usize>, Option<usize>) {
    fn walk(
        nodes: &[Node],
        at: &mut usize,
        map: &mut std::collections::HashMap<u64, usize>,
        last: &mut Option<usize>,
    ) {
        for n in nodes {
            let Node::Element(e) = n else { continue };
            if unkeyed_positioned(e) {
                *last = Some(*at);
            }
            *at += 1;
            walk(&e.children, at, map, last);
            // Конец поддерева: свои потомки порядок не ломают — они рисуются
            // вместе с элементом.
            map.insert(e.node_id, *at);
        }
    }
    let mut map = std::collections::HashMap::new();
    let mut last = None;
    walk(nodes, &mut 0, &mut map, &mut last);
    (map, last)
}

/// Можно ли поднять краску элемента в собиратель шага 8: ПОЗЖЕ по документу
/// нет позиционированного, который останется в первом проходе (иначе
/// порядок разметки перевернётся — `position-relative-table-*`: ячейка
/// `relative` после абсолютного красного индикатора). Узел вне обхода
/// (порождённый сборщиком) — по братьям, как прежде.
/// Whether an inline absolute box with `z-index: auto | 0` goes to the
/// step-8 collector: it is a known node of this document walk and no later
/// positioned box stays in the first paint pass (`unkeyed_positions`).
pub(crate) fn hoists_inline_abs(e: &Element, cx: &LayoutCx) -> bool {
    let u = &*cx.unkeyed;
    let known =
        u.0.get(&e.node_id)
            .map(|&end| u.1.is_none_or(|last| last < end));
    e.style.used().z_index.unwrap_or(0) == 0 && known == Some(true)
}

pub(crate) fn paint_last_ok(e: &Element, rest: &[Node], cx: &LayoutCx) -> bool {
    let u = &*cx.unkeyed;
    let known =
        u.0.get(&e.node_id)
            .map(|&end| u.1.is_none_or(|last| last < end));
    match known {
        Some(ok) => ok,
        None => !positioned_later(rest),
    }
}
