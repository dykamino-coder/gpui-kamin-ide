//! Предикаты коробок: строчные, блочные, в потоке, свой контекст, замещаемые.

mod line_boxes;
pub(crate) use line_boxes::holds_line_box;
pub(crate) use line_boxes::inline_level;
pub(crate) use line_boxes::inline_level_box;
pub(crate) use line_boxes::phantom_inline;
pub(crate) use line_boxes::replaced_tag;

mod contexts;
pub(crate) use contexts::own_context;
pub(crate) use contexts::own_context_style;

use crate::dom::{Element, Node};
use crate::style::computed::{Computed, Display};
use crate::text::text_box::blank_text;

/// Обтекание: плавающий блок и следующие за ним встают в один ряд.
///
/// Своего обтекания в раскладке нет и быть не может — оно определено через
/// строчный контекст, которого taffy не знает. Но ровно то, ради чего его
/// пишут — «картинка слева, текст справа» — выражается рядом из двух колонок
/// точно. Отличие от браузера одно: текст не заворачивается ПОД плавающий
/// блок, когда тот кончился. `clear` закрывает ряд и начинает новый.
/// Уходит ли элемент из потока: плавающие и внепоточные строчного не рвут
/// (Blink `layout_inline.cc`: разрыв вызывают только блоки В ПОТОКЕ).
pub(crate) fn out_of_flow(c: &Computed) -> bool {
    c.float.is_some_and(|f| f != 0)
        || matches!(
            c.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
}

/// Настоящий ли это строчный элемент.
///
/// `display: inline` хранится как строчная коробка с пометкой — по одному
/// лишь тегу судить нельзя: `<div style="display:inline">` строчный, а
/// `<span style="display:block">` блочный.
pub(crate) fn real_inline(e: &Element) -> bool {
    // Атомарные строчные — кнопка, поле, список выбора и замещаемые — стоят
    // в строке целиком, и содержимое их не разрывает: рвутся только
    // НЕзамещаемые строчные коробки (CSS 2.1 §9.2.1.1).
    const ATOMIC: &[&str] = &[
        "button", "select", "textarea", "input", "img", "svg", "canvas", "video", "audio",
        "object", "embed", "iframe", "meter", "progress",
    ];
    if ATOMIC.contains(&e.tag.as_str()) {
        return false;
    }
    if e.style.inline_display == Some(true) {
        return true;
    }
    match e.style.display {
        Some(_) => false,
        None => e.inline || crate::dom::INLINE_TAGS.contains(&e.tag.as_str()),
    }
}

/// Блочный ли это узел с точки зрения разрыва строчного.
pub(crate) fn breaks_inline(n: &Node) -> bool {
    let Node::Element(e) = n else { return false };
    block_level_in_flow(e)
}

/// Внутрипоточная коробка БЛОЧНОГО уровня (CSS 2.1 §9.2.1): не флоат и не
/// абсолют, не настоящая строчная, и вид — блочный (`display`, а при пустом
/// `display` — блочный тег). Ею же решается, кто спаннер (`spanner_box`:
/// css-multicol-1 §column-span «Applies to: in-flow block-level elements»).
pub(crate) fn block_level_in_flow(e: &Element) -> bool {
    if out_of_flow(&e.style) || real_inline(e) {
        return false;
    }
    // Рвут строку только НАСТОЯЩИЕ блочные виды. Внутренние части таблицы
    // (ряд, ячейка, группа) сами по себе разрыва не вызывают: вокруг них
    // сборщик строит анонимную таблицу, и её судьба решается отдельно.
    match e.style.display {
        // Строчные лунки строку НЕ рвут — внешний вид у них `inline`.
        Some(Display::GridLanes) => !e.style.lanes_inline,
        Some(Display::Block)
        | Some(Display::Flex)
        | Some(Display::Grid)
        | Some(Display::Table)
        | Some(Display::ListItem) => true,
        Some(_) => false,
        None => !e.inline && !crate::dom::INLINE_TAGS.contains(&e.tag.as_str()),
    }
}

/// Есть ли в поддереве строчного блочный потомок в потоке.
///
/// `display: contents` своей коробки не даёт — блок из-под него виден
/// строчному хозяину как свой (css-display-3 §box-generation).
pub(crate) fn contains_block(children: &[Node]) -> bool {
    children.iter().any(|n| match n {
        Node::Element(e) if real_inline(e) || e.style.display == Some(Display::Contents) => {
            !out_of_flow(&e.style) && contains_block(&e.children)
        }
        other => breaks_inline(other),
    })
}

pub(crate) fn is_blank(n: &Node) -> bool {
    matches!(n, Node::Text(t) if blank_text(t))
}

/// Схлопываются ли отступы этого элемента с соседями и родителем.
///
/// Схлопывание — свойство БЛОЧНОГО потока. Не схлопываются: плавающий блок,
/// абсолютный и всё строчного уровня (`inline-block` и родня) — у них поля
/// стоят как написаны. Без этой проверки поле плавающего ребёнка «протекало»
/// наружу и поднимало родителя, а ряд строчных коробок терял поля у всех,
/// кроме первой.
pub(crate) fn in_flow(c: &Computed) -> bool {
    c.float.is_none()
        && !matches!(
            c.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
        && !matches!(
            c.display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
}

/// Замещаемый строчный атом: своих детей не имеет, но КОРОБКУ рождает —
/// значит, рождает и строчную коробку. Пустой `<span>` — не рождает.
pub(crate) fn replaced_inline(tag: &str) -> bool {
    matches!(
        tag,
        "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe" | "input" | "br"
    )
}

/// Строчного УРОВНЯ, но В ПОТОКЕ: `inline-block` и родня. Рождает строчную
/// коробку, в отличие от плавающего и абсолютного, которых в потоке нет.
/// Разбор держит `display: inline` как `InlineBlock` с пометкой
/// `inline_display`, поэтому одного взгляда на `display` мало.
pub(crate) fn atomic_inline(c: &Computed) -> bool {
    c.float.is_none()
        && !matches!(
            c.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
        && c.inline_display != Some(true)
        && matches!(
            c.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable)
        )
}

/// Блочная коробка со строчной ПОМЕТКОЙ: псевдоэлемент (`::before`/`::after`
/// помечается строчным независимо от `display`, `dom.rs`) или строчный тег с
/// блочным `display`. Раскладка (`breaks_inline`) кладёт такую коробку
/// блоком, а цепочки схлопывания полей пропускали её как строчную — и
/// `::after { display: flow-root; margin-top: 200px }` оставлял поле внутри
/// родителя вместо примыкания к его верху (`phantom-line-boxes-001…006`).
pub(crate) fn inline_marked_block(e: &Element) -> bool {
    e.inline
        && e.style.inline_display != Some(true)
        && matches!(
            e.style.display,
            Some(Display::Block)
                | Some(Display::ListItem)
                | Some(Display::Flex)
                | Some(Display::Grid)
                | Some(Display::Table)
        )
}
