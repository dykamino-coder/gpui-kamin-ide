//! Порядок отложенной окраски позиционированных inline-коробок.

use super::next_paint_key;
use super::{UNKEYED, block_level_in_flow};
use crate::dom::{Element, Node};
use crate::layout::positioned::predicates::positioned_later;
use crate::style::computed::Display;
use gpui::{AnyElement, IntoElement};

/// Позиционированный с `z-index: auto | 0`, которого сборщик не оборачивает
/// в `PaintLast`: части таблицы (их строит табличный сборщик),
/// `relative`/`sticky` строчного уровня и строчный абсолют — те идут в
/// абзац. Такие красятся первым проходом там, где стоят.
pub(super) fn unkeyed_positioned(e: &Element) -> bool {
    if e.style.z_index.unwrap_or(0) != 0 {
        return false;
    }
    let table_part = matches!(
        e.style.display,
        Some(Display::TableRow) | Some(Display::TableCell) | Some(Display::TableRowGroup)
    ) || (e.style.display.is_none()
        && matches!(
            e.tag.as_str(),
            "tr" | "td" | "th" | "tbody" | "thead" | "tfoot" | "caption" | "col" | "colgroup"
        ));
    match e.style.position {
        Some(crate::style::computed::Position::Relative)
        | Some(crate::style::computed::Position::Sticky) => table_part || !block_level_in_flow(e),
        Some(crate::style::computed::Position::Absolute) => {
            table_part || (e.style.display.is_none() && e.inline)
        }
        _ => false,
    }
}

/// Прямой обход документа: конец поддерева каждого узла (позиция за его
/// последним потомком) и позиция последнего позиционированного без ключа
/// краски.
pub(super) fn unkeyed_positions(
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
pub(super) fn paint_last_ok(e: &Element, rest: &[Node]) -> bool {
    let known = UNKEYED.with(|u| {
        let u = u.borrow();
        u.0.get(&e.node_id)
            .map(|&end| u.1.is_none_or(|last| last < end))
    });
    match known {
        Some(ok) => ok,
        None => !positioned_later(rest),
    }
}

/// Строчный абсолют с `z-index: auto | 0` в позднем слое (`late_push`): шаг 8
/// приложения E CSS 2.1 — позиционированные рисуются ПОСЛЕ строчного
/// содержимого (шаг 7) своего контекста наложения. Строки абзаца уходят в
/// собиратель (`PaintInline`) и рисуются в его конце, а поздний слой — прямой
/// ребёнок контейнера и красился раньше них: текст ложился поверх абсолюта
/// (`ch-unit-001`, `ic-unit-001`). `PaintLast` ставит коробку в собиратель по
/// ключу в порядке разметки. Узел вне обхода (порождённый сборщиком) остаётся
/// на прежнем пути.
pub(crate) fn inline_abs_paint_last(e: &Element, el: AnyElement) -> AnyElement {
    let known = UNKEYED.with(|u| {
        let u = u.borrow();
        u.0.get(&e.node_id)
            .map(|&end| u.1.is_none_or(|last| last < end))
    });
    if e.style.z_index.unwrap_or(0) == 0 && known == Some(true) {
        gpui::PaintLast::new(el)
            .key(next_paint_key())
            .into_any_element()
    } else {
        el
    }
}
