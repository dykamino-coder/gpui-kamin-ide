//! Потоки, которым независимой сплошной копии недостаточно до ADR P-19 F2.

use crate::dom::{Element, Node};
use crate::style::computed::Display;
use crate::style::values::value::Len;

pub(super) fn continuation(e: &Element) -> bool {
    if e.style.computed().display == Some(Display::None) {
        return false;
    }
    // TRANSITIONAL(F2-4): flex_breaks экспортирует только первый монолит; удалить отказ в P14 после consumed block size и отдельных продолжений flex items.
    super::super::blocks::flex_container(e)
        // TRANSITIONAL(F2-1): Один border box не хранит остаток min-height и overflow; удалить отказ в P04 после consumed block size и раздельных продолжений.
        || parallel_overflow(e)
        // TRANSITIONAL(F2-1): MaxContent копии теряет исходный definite CB для процентов; удалить отказ в P04 после сохранения исходного CB в продолжении.
        || [e.style.computed().height, e.style.computed().min_height, e.style.computed().max_height]
            .iter().any(|l| matches!(l, Some(Len::Pct(_))))
        || inline_continuation(e)
        || e.children.iter().any(|n| matches!(n, Node::Element(k) if continuation(k)))
}

pub(super) fn inline_continuation(e: &Element) -> bool {
    // TRANSITIONAL(F2-6): Initial-letter и ruby требуют собственных краёв и строчных продолжений; удалить отказ в P20 после inline tokens поверх P03.
    e.first_letter
        .as_ref()
        .is_some_and(|s| s.computed().initial_letter.is_some())
        || e.tag == "ruby"
        || e.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if inline_continuation(k)))
}

/// TRANSITIONAL(F2-1): Очистка детей теряет независимый прогресс normal flow, min-height и overflow; удалить gate в P05 после отдельных resume этих потоков.
pub(crate) fn parallel_overflow(e: &Element) -> bool {
    crate::layout::page::paged::visible_overflow(&e.style.computed())
        && [
            e.style.computed().height,
            e.style.computed().min_height,
            e.style.computed().max_height,
        ]
        .iter()
        .any(|l| !matches!(l, None | Some(Len::Auto)))
        && e.children.iter().any(|n| matches!(n, Node::Element(_)))
}
