//! Потоки, которым независимой сплошной копии недостаточно до ADR P-19 F2.

use crate::dom::{Element, Node};
use crate::style::computed::Display;

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
    // TRANSITIONAL(F2-6): Ruby ломает строку после контейнера руби без точки переноса (css-ruby-1 §2.4, multicol-fill-balance-031); удалить отказ после исправления переноса руби.
    e.tag == "ruby"
        || e.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if inline_continuation(k)))
}
