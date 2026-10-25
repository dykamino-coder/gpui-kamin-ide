//! Потоки, которым независимой сплошной копии недостаточно до ADR P-19 F2.

use crate::dom::{Element, Node};
use crate::style::computed::Display;
use crate::style::values::value::Len;

/// Наличие заданного размера и детей; отдельный допуск float_copy использует
/// это свойство до появления продолжения обтекаемого BFC в P06.
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

pub(super) fn continuation(e: &Element) -> bool {
    if e.style.used().display == Some(Display::None) {
        return false;
    }
    inline_continuation(e)
        || e.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if continuation(k)))
}

pub(super) fn inline_continuation(e: &Element) -> bool {
    // TRANSITIONAL(F2-6): Initial-letter и ruby требуют собственных краёв и строчных продолжений; удалить отказ в P20 после inline tokens поверх P03.
    e.first_letter
        .as_ref()
        .is_some_and(|s| s.used().initial_letter.is_some())
        || e.tag == "ruby"
        || e.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if inline_continuation(k)))
}
