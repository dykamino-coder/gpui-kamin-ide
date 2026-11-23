//! Предикаты позиционирования для сборки дерева: остаётся ли дальше по разметке позиционированный
//! сосед, с которым вынос в слой нарушил бы порядок шага 8 CSS 2.1 §9.9 (`positioned_later`,
//! `stays_positioned`), задан ли край (`edge_set`) и есть ли своя коробка (`has_own_box`).

use crate::dom::{Children, Element, Node, PositionedQuery};
use crate::layout::box_kind::block_level_in_flow;
use crate::style::computed::Position;
use crate::style::values::value::Len;
mod own_box;
mod siblings;
pub(crate) use own_box::has_own_box;
pub(crate) use siblings::{Later, Siblings};

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
/// `PaintLast` без этого гейта ломает все `position-relative-table-*` и
/// `font-029`.
///
/// The answer of one node of the list; Later::positioned_later answers for
/// all nodes after one.
fn later_node(e: &Element) -> bool {
    let style = e.style.used();
    let pos = style.reset_positioned.position;
    let late_sibling = matches!(pos, Some(Position::Relative) | Some(Position::Sticky))
        && style.reset_positioned.z_index.unwrap_or(0) == 0
        && block_level_in_flow(e);
    !late_sibling && (is_positioned(pos) || positioned_within(&e.children))
}

/// Any positioned element in the subtrees of `children`; the answer lives in
/// the children snapshot, so each list of the tree is walked once.
fn positioned_within(children: &Children) -> bool {
    children.positioned_fact(PositionedQuery::Positioned, |nodes| {
        nodes.iter().any(|n| {
            let Node::Element(e) = n else { return false };
            positioned_own(e) || positioned_within(&e.children)
        })
    })
}

fn is_positioned(pos: Option<Position>) -> bool {
    matches!(
        pos,
        Some(Position::Relative) | Some(Position::Sticky) | Some(Position::Absolute)
    )
}

// Out of line: the style copy stays off the frames of the recursive walk.
#[inline(never)]
fn positioned_own(e: &Element) -> bool {
    is_positioned(e.style.used().reset_positioned.position)
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
///
/// The answer of one node and its subtree; Later::stays_positioned answers
/// for all nodes after one.
fn stays_node(e: &Element, under_cb: bool) -> bool {
    let under_cb = match stays_own(e, under_cb) {
        Ok(answer) => return answer,
        Err(under_cb) => under_cb,
    };
    let query = if under_cb {
        PositionedQuery::StaysUnderCb
    } else {
        PositionedQuery::StaysOutsideCb
    };
    // The answer for the subtree lives in its children snapshot: each list
    // of the tree is walked once per question.
    e.children.positioned_fact(query, |nodes| {
        nodes
            .iter()
            .any(|n| matches!(n, Node::Element(c) if stays_node(c, under_cb)))
    })
}

/// The node's own answer, or `Err` with the `under_cb` of its children when
/// the answer is in the subtree. Out of line: the style copy stays off the
/// frames of the recursive walk.
#[inline(never)]
fn stays_own(e: &Element, under_cb: bool) -> Result<bool, bool> {
    let style = e.style.used();
    let pos = style.reset_positioned.position;
    let positioned = is_positioned(pos);
    let z = style.reset_positioned.z_index;
    // The same predicate as the hoist: such a sibling moves to the layer,
    // and the mutual order is kept.
    let inset = &style.reset_positioned.inset;
    let hoisted = pos == Some(Position::Absolute)
        && !under_cb
        && z.unwrap_or(0) >= 0
        && (edge_set(inset.left.clone())
            || edge_set(inset.right.clone())
            || edge_set(inset.top.clone())
            || edge_set(inset.bottom.clone()));
    // A negative `z-index` paints in the bottom layer (CSS 2.1 §9.9,
    // step 3) whatever its document position: hoisting an earlier
    // sibling cannot reorder against it (spec-examples
    // `shape-outside-001`: `#failure-container` kept `#test` in
    // place, positioned from the collapsed `body` top, 16px low).
    if positioned && z.is_some_and(|z| z < 0) {
        // …together with its whole subtree.
        return Ok(false);
    }
    if positioned && !hoisted {
        return Ok(true);
    }
    // A hoisted sibling moves to the layer TOGETHER with its subtree: its
    // positioned descendants paint inside it and keep the order with the
    // hoisted one. Before, an absolute child of such a sibling kept the
    // element in place, and its edges were taken from `body`, not from the
    // viewport (`backdrop-filters-*`: the square moved onto the body margin).
    if hoisted {
        return Ok(false);
    }
    Err(under_cb || crate::text::inline::establishes_cb(&style))
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
