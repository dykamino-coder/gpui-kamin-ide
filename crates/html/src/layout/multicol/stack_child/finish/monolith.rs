//! Monolithic descendants and indivisible column-copy classification.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::fragment::probe::size_monolith;
use crate::style::computed::Display;

/// A monolithic descendant overflows its fragmentainer instead of taking
/// space in the flow (css-break-3 §4.1), so the unfragmented height no
/// longer counts the columns the content needs.
pub(super) fn has_monolith(c: &Element, depth: u8) -> bool {
    depth > 0
        && c.children.iter().any(|n| match n {
            Node::Element(k) => {
                crate::layout::fragment::clone::monolith_box(k) || has_monolith(k, depth - 1)
            }
            _ => false,
        })
}

pub(super) fn copy_monolith(
    copy: &Element,
    cuts: &[(f32, f32)],
    nest_row: Option<f32>,
) -> (
    impl Fn(Option<crate::style::computed::Overflow>) -> bool + use<>,
    bool,
) {
    let scrolls = |o: Option<crate::style::computed::Overflow>| {
        matches!(o, Some(crate::style::computed::Overflow::Scroll))
    };
    let block_kid = |n: &Node| {
        matches!(n, Node::Element(k)
            if !k.inline || k.style.display == Some(Display::Block))
    };
    // Size containment uses the same monolith predicate as fragment
    // measurement and grow_pushed. Nested row copies remain fragmentable.
    let monolith = nest_row.is_none()
        && (size_monolith(copy)
        || copy.style.break_inside_avoid
        || scrolls(copy.style.overflow_x)
        || scrolls(copy.style.overflow_y)
        || matches!(
            copy.tag.as_str(),
            "img"
                | "svg"
                | "canvas"
                | "video"
                | "embed"
                | "object"
                | "iframe"
        )
        // Таблица и ячейка — не монолиты
        // (css-break-4 §4.1).
        || matches!(
            copy.style.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
        )
        // Сплошной СТРОЧНЫЙ набор тоже монолит:
        // резать его можно лишь между строками, а
        // строк укладка колонок не видит, и разрез
        // приходился бы посреди строки.
        // ПУСТАЯ коробка с высотой режется по своей
        // высоте (css-break-4 §4.2; корень A3).
        || (copy.children.iter().any(|n| !is_blank(n))
            && !copy.children.iter().any(block_kid)
            // Строки измерены (`line_run_shape`) — режется
            // между строк, не монолит.
            && cuts.is_empty()));
    (scrolls, monolith)
}
