//! Классификация inline-коробок и участия в line box.

use super::{atomic_inline, inline_marked_block, replaced_inline};
use crate::dom::{Element, Node};
use crate::layout::block::struts::inline_axis_edges;
use crate::style::computed::Display;
use crate::text::text_box::blank_text;

/// Содержит ли коробка строчную коробку (§8.3.1, «does not contain a line
/// box»; нулевые строчные коробки §9.4.2 не в счёт).
///
/// Правила те же, что у `first_in_flow`: пробельный текст прозрачен,
/// непробельный рождает строку; ПУСТОЙ строчный элемент прозрачен, а
/// замещаемый атом (`img` и родня) — нет; вне потока строки не рождает никто;
/// `display: contents` своей коробки не даёт — смотреть надо в его детей.
pub(crate) fn holds_line_box(children: &[Node]) -> bool {
    children.iter().any(|n| match n {
        Node::Text(t) => !blank_text(t),
        Node::Element(ch) => {
            if ch.style.display == Some(Display::None) {
                return false;
            }
            if ch.style.display == Some(Display::Contents) {
                return holds_line_box(&ch.children);
            }
            // Плавающий и абсолютный строчной коробки не рождают.
            if ch.style.float.is_some_and(|f| f != 0)
                || matches!(
                    ch.style.position,
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                )
            {
                return false;
            }
            // Атомарный строчный в потоке — своя строчная коробка. Проверять
            // ДО `in_flow`: он их не различает и валит в одну корзину с
            // плавающим.
            if atomic_inline(&ch.style) {
                return true;
            }
            // `display: inline` делает строчным ЛЮБОЙ тег: своей коробки у
            // него нет, а строчную рождает его содержимое. Без этой ветки
            // `<div style="display:inline">` считался блочным ребёнком и
            // строки «не рождал», хотя текст внутри него её рождает.
            if ch.inline || ch.style.inline_display == Some(true) {
                // Блочный псевдоэлемент — блочный ребёнок: строки родителю
                // не рождает, его содержимое разбирает `through_strut`.
                if inline_marked_block(ch) {
                    return false;
                }
                // Пустой строчный с ненулевым полем/отступом/рамкой по
                // строчной оси — не фантом (css-inline-3
                // §invisible-line-boxes): строка есть, схлопывание насквозь
                // закрыто (`phantom-line-boxes-001…006`).
                return replaced_inline(&ch.tag)
                    || inline_axis_edges(&ch.style)
                    || holds_line_box(&ch.children);
            }
            // Блочный ребёнок строки не рождает: его содержимое разбирает
            // рекурсия `through_strut`.
            false
        }
    })
}

/// Ведущая цепочка примыкания к ВЕРХНЕМУ краю коробки (§8.3.1).
///
/// Верхнее поле коробки примыкает к верхнему полю её первого ребёнка в потоке.
/// Если тот схлопывается насквозь, примыкание тянется ВБОК, к следующему
/// брату; если не схлопывается — ВГЛУБЬ, к его собственному первому ребёнку.
///
/// Меряет ИММУТАБЕЛЬНО и копит пути до съеденных полей: обнулять на ходу
/// нельзя, потому что доля или `calc` на середине цепи заставят вернуть
/// `None`, а записанные нули уже не откатить.
/// Пустой строчный элемент без краёв по строчной оси — фантом
/// (css-inline-3 §invisible-line-boxes): строки не рождает, примыкания не
/// рвёт. Тот же признак, что у `leading_chain`.
pub(crate) fn phantom_inline(n: &Node) -> bool {
    matches!(n, Node::Element(s) if s.inline
        && !inline_marked_block(s)
        && s.children.is_empty()
        && !replaced_inline(&s.tag)
        && !inline_axis_edges(&s.style)
        && !atomic_inline(&s.style))
}

/// Строчный ли элемент по своему `display`.
pub(crate) fn inline_level(e: &Element) -> bool {
    match e.style.display {
        Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid) => true,
        // Строчный контейнер лунок — атом в строке, как inline-grid
        // (grid-lanes-align-content-001: четыре сетки стоят В РЯД).
        Some(Display::GridLanes) => e.style.lanes_inline,
        Some(_) => false,
        None => e.inline,
    }
}

/// Размер по содержимому (`width: min-content`/`max-content`).
///
/// У коробки такого размера раскладка под нами не знает — зато знает такую
/// ДОРОЖКУ СЕТКИ. Элемент заворачивается в сетку из одной дорожки нужного
/// вида: ширину она посчитает по содержимому и отдаст элементу. Обёртка
/// прижата к началу строки, иначе сетка растянула бы её саму на всю ширину
/// родителя и смысл потерялся.
/// Завернёт ли `content_sized` этот элемент в свою обёртку.
///
/// Отдельный предикат нужен вызывающей стороне: она обязана снять с элемента
/// боковые поля ДО сборки — обёртка их не пропускает.
/// Замещаемый элемент (css-display-3 §2.4): размер даёт содержимое, а не
/// раскладка детей.
pub(crate) fn replaced_tag(e: &Element) -> bool {
    matches!(
        e.tag.as_str(),
        "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe" | "input"
    )
}

/// Текст ДО первого жёсткого разрыва: дальше первая строка не идёт никогда.
///
/// Замер первой строки ищет, сколько знаков влезет по ширине, и про `<br>` он
/// не знает — с широкой коробкой в первую строку попадал весь абзац, и её
/// начертание доставалось второй строке тоже
/// (`text-autospace-first-line-001`).
/// Уровень коробки для схлопывания полей: тег — только УМОЛЧАНИЕ, вид из
/// каскада сильнее. `e.inline` ставится по имени тега (`dom.rs`), поэтому
/// `<span style="display:block">` доезжал сюда «строчным», и поля соседей
/// через него не примыкали — эталоны `flex-direction-column*` разводило на
/// лишние 16 точек (в них разметка именно такая).
pub(crate) fn inline_level_box(e: &Element) -> bool {
    match e.style.display {
        // Строчными считаются только НАСТОЯЩИЕ строчные виды. Первый заход
        // писал `Some(_) => true`, и в строчные попадали лунки сетки: CSS2
        // +6, а css-grid −46 (`column-align-items-*`, `*-dense-packing-*`
        // уходили 0.00 → 1.5-3.2). `display: inline` после каскада — это
        // `InlineBlock` с пометкой `inline_display` (см. `computed.rs`).
        Some(Display::InlineBlock)
        | Some(Display::InlineFlex)
        | Some(Display::InlineGrid)
        | Some(Display::InlineTable) => true,
        Some(_) => false,
        None => e.inline,
    }
}
