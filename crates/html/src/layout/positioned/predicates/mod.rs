//! Предикаты позиционирования.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::page::paged::visible_overflow;
use crate::render::block_level_in_flow;
use crate::style::values::value::Len;
mod own_box;
pub(crate) use own_box::has_own_box;

// Несёт ли поддерево АБСОЛЮТНОГО потомка, чей низ `shape_full` сворачивает
// в меру коробки (дотяг `oof_reach`). Только такому ребёнку стопки колонок
// переполняющие колонки нужны ради внепоточного (css-position-3
// §abspos-breaking: «The box may subsequently be broken over several
// fragmentation containers»; Blink рождает их от внепоточного —
// `column_layout_algorithm.cc` `num_new_columns`). Спуск не идёт внутрь
// коробки, обрезающей переполнение (абсолют за ней в колонках не виден:
// `out-of-flow-in-multicolumn-107`, `overflow: clip` над абсолютом
// 100000px), и внутрь вложенного многоколоночника (у его абсолютов свои
// колонки). Глубина — та же, что у меры стопки (`shape_full(c, 4, ..)`).
thread_local! {
    /// Коробки, чью меру фрагментации дотянули внепоточные потомки
    /// (`shape_full`): `node_id -> (свой размер, мера с дотягом)`.
    pub(crate) static OOF_OWN: std::cell::RefCell<std::collections::HashMap<u64, (f32, f32)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub(crate) fn carries_abspos(c: &Element, depth: u8) -> bool {
    depth > 0
        && c.children.iter().any(|n| match n {
            Node::Element(k) => {
                k.style.position == Some(crate::style::computed::Position::Absolute)
                    || (visible_overflow(&k.style)
                        && !multicol_container(&k.style)
                        && carries_abspos(k, depth - 1))
            }
            _ => false,
        })
}

/// Рисуется ли ДАЛЬШЕ по разметке позиционированное содержимое первым
/// проходом родителя — тогда `PaintLast` перевернул бы порядок шага 8.
///
/// Шаг 8 приложения E CSS 2.1 красит позиционированные потомки контекста В
/// ПОРЯДКЕ РАЗМЕТКИ, а второй проход `Div` поднимает обёрнутого лишь над
/// братьями: позиционированный ВНУТРИ следующего обычного брата (ячейка
/// `relative` в таблице после абсолютного красного индикатора,
/// `position-relative-table-*`) или абсолют на статической позиции в позднем
/// слое (`font-029`) оказались бы под ним. Брат-блок `relative`/`sticky` с
/// `z-index: auto | 0` сам уходит во второй проход и порядок сохраняет — его
/// поддерево рисуется вместе с ним. `fixed` отложен и так.
///
/// ★ ЗАМЕРЕНО И ОТКАЧЕНО: `PaintLast` без этого гейта — на срезе 890 пар
/// +2/−20 (все `position-relative-table-*` и `font-029` в «красное видно»);
/// с гейтом срез 3483 пары: 2757 → 2764, +7/−0.
pub(crate) fn positioned_later(rest: &[Node]) -> bool {
    fn positioned(e: &Element) -> bool {
        matches!(
            e.style.position,
            Some(crate::style::computed::Position::Relative)
                | Some(crate::style::computed::Position::Sticky)
                | Some(crate::style::computed::Position::Absolute)
        )
    }
    fn walk(nodes: &[Node]) -> bool {
        nodes.iter().any(|n| {
            let Node::Element(e) = n else { return false };
            positioned(e) || walk(&e.children)
        })
    }
    rest.iter().any(|n| {
        let Node::Element(e) = n else { return false };
        let late_sibling = matches!(
            e.style.position,
            Some(crate::style::computed::Position::Relative)
                | Some(crate::style::computed::Position::Sticky)
        ) && e.style.z_index.unwrap_or(0) == 0
            && block_level_in_flow(e);
        if late_sibling {
            return false;
        }
        positioned(e) || walk(&e.children)
    })
}

/// Есть ли ДАЛЬШЕ по разметке позиционированный элемент, который останется на
/// месте.
///
/// Слой начального содержащего блока дописывается последним ребёнком
/// документа, поэтому вынесенный рисуется поверх всего, что осталось в потоке.
/// По CSS 2.1 §9.9 шаг 8 позиционированные с `z-index: auto` рисуются В
/// ПОРЯДКЕ РАЗМЕТКИ: сосед, стоящий ПОСЛЕ, обязан лечь СВЕРХУ. Пока он
/// остаётся на месте, вынос переворачивает пару местами.
///
/// Сосед, который сам уйдёт в слой, порядок НЕ ломает: слой копится в порядке
/// сборки. `fixed` не считается: он и так рисуется отложенно, поверх всего.
pub(crate) fn stays_positioned(rest: &[Node]) -> bool {
    fn walk(nodes: &[Node], under_cb: bool) -> bool {
        nodes.iter().any(|n| {
            let Node::Element(e) = n else { return false };
            let pos = e.style.position;
            let positioned = matches!(
                pos,
                Some(crate::style::computed::Position::Relative)
                    | Some(crate::style::computed::Position::Sticky)
                    | Some(crate::style::computed::Position::Absolute)
            );
            // Тот же предикат, что и у выноса: такой сосед уедет в слой, и
            // взаимный порядок сохранится.
            let hoisted = pos == Some(crate::style::computed::Position::Absolute)
                && !under_cb
                && e.style.z_index.unwrap_or(0) >= 0
                && (edge_set(e.style.inset.left)
                    || edge_set(e.style.inset.right)
                    || edge_set(e.style.inset.top)
                    || edge_set(e.style.inset.bottom));
            // A negative `z-index` paints in the bottom layer (CSS 2.1 §9.9,
            // step 3) whatever its document position: hoisting an earlier
            // sibling cannot reorder against it (spec-examples
            // `shape-outside-001`: `#failure-container` kept `#test` in
            // place, positioned from the collapsed `body` top, 16px low).
            let below = e.style.z_index.is_some_and(|z| z < 0);
            if positioned && below {
                // …together with its whole subtree.
                return false;
            }
            if positioned && !hoisted {
                return true;
            }
            // Вынесенный сосед уезжает в слой ВМЕСТЕ с поддеревом: его
            // позиционированные потомки рисуются внутри него и порядок с
            // выносимым не ломают. Прежде абсолютный ребёнок такого соседа
            // держал элемент на месте, и края считались от `body`, а не от
            // окна (`backdrop-filters-*`: квадрат съезжал на поле тела).
            if hoisted {
                return false;
            }
            walk(
                &e.children,
                under_cb || crate::text::inline::establishes_cb(&e.style),
            )
        })
    }
    walk(rest, false)
}

/// Задан ли край позиционированного элемента.
///
/// `left: auto` — это ОТСУТСТВИЕ края (CSS 2.1 §9.3.2: начальное значение
/// `auto`), а разбор даёт на него `Some(Len::Auto)`. Проверка `is_some()`
/// читала явный `auto` как заданный край, и элемент терял статическую
/// позицию: `abspos-*-applies-to-*` вставали в угол содержащего блока
/// вместо своего места в потоке.
pub(crate) fn edge_set(l: Option<Len>) -> bool {
    !matches!(l, None | Some(Len::Auto))
}
