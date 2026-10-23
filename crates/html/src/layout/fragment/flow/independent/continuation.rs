//! Потоки, которым независимой сплошной копии недостаточно до ADR P-19 F2.

use crate::dom::{Element, Node};
use crate::style::computed::Display;
use crate::style::values::value::Len;

pub(super) fn continuation(e: &Element) -> bool {
    if e.style.display == Some(Display::None) {
        return false;
    }
    // F2-4: flex_breaks экспортирует лишь первый монолит потока. Нужны
    // consumed block size и отдельные продолжения каждого flex item.
    super::super::blocks::flex_container(e)
        // F2-1: ограниченный блок и видимое переполнение продолжаются
        // независимо; один border box не заменяет обе меры.
        || parallel_overflow(e)
        // F2-1: доля блочного размера зависит от исходного definite CB,
        // который замер MaxContent отдельной копии не сохраняет.
        || [e.style.height, e.style.min_height, e.style.max_height]
            .iter().any(|l| matches!(l, Some(Len::Pct(_))))
        || inline_continuation(e)
        || e.children.iter().any(|n| matches!(n, Node::Element(k) if continuation(k)))
}

pub(super) fn inline_continuation(e: &Element) -> bool {
    // F2-1/F2-6: отдельная initial-letter-коробка и ruby-аннотация имеют
    // собственные края/строчные продолжения; обычный рекордер строк их не хранит.
    e.first_letter
        .as_ref()
        .is_some_and(|s| s.initial_letter.is_some())
        || e.tag == "ruby"
        || e.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if inline_continuation(k)))
}

/// F2-1: normal flow и видимое переполнение ограниченной коробки
/// продолжаются отдельно; очистка детей до допуска не должна терять это.
pub(crate) fn parallel_overflow(e: &Element) -> bool {
    crate::layout::page::paged::visible_overflow(&e.style)
        && [e.style.height, e.style.max_height]
            .iter().any(|l| !matches!(l, None | Some(Len::Auto)))
        && e.children.iter().any(|n| matches!(n, Node::Element(_)))
}
