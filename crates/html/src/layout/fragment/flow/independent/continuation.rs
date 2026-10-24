//! Потоки, которым независимой сплошной копии недостаточно до ADR P-19 F2.

use crate::dom::{Element, Node};
use crate::style::computed::Display;
use crate::style::values::value::Len;

pub(super) fn continuation(e: &Element) -> bool {
    if e.style.used().display == Some(Display::None) {
        return false;
    }
    // TRANSITIONAL(F2-1): ограничения блочного размера требуют consumed
    // size; один border box не хранит остаток min-height и переполнение.
    parallel_overflow(e)
        // TRANSITIONAL(F2-1): доля блочного размера зависит от исходного definite CB,
        // который замер MaxContent отдельной копии не сохраняет.
        || [e.style.used().height, e.style.used().min_height, e.style.used().max_height]
            .iter().any(|l| matches!(l, Some(Len::Pct(_))))
        || inline_continuation(e)
        || e.children.iter().any(|n| matches!(n, Node::Element(k) if continuation(k)))
}

pub(super) fn inline_continuation(e: &Element) -> bool {
    // TRANSITIONAL(F2-1/F2-6): отдельная initial-letter-коробка и ruby-аннотация имеют
    // собственные края/строчные продолжения; обычный рекордер строк их не хранит.
    e.first_letter
        .as_ref()
        .is_some_and(|s| s.used().initial_letter.is_some())
        || e.tag == "ruby"
        || e.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if inline_continuation(k)))
}

/// TRANSITIONAL(F2-1): normal flow, остаток min-height и переполнение
/// продолжаются отдельно; очистка детей до допуска не должна терять это.
pub(crate) fn parallel_overflow(e: &Element) -> bool {
    crate::layout::page::paged::visible_overflow(&e.style.used())
        && [
            e.style.used().height,
            e.style.used().min_height,
            e.style.used().max_height,
        ]
        .iter()
        .any(|l| !matches!(l, None | Some(Len::Auto)))
        && e.children.iter().any(|n| matches!(n, Node::Element(_)))
}
