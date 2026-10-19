//! @supports: условия, селекторы, трёхзначная логика.

use crate::document::state::ValueCx;
use crate::style::css::*;

mod at_rule;
#[cfg(test)]
mod declaration_tests;
#[cfg(test)]
mod property_tests;
#[cfg(test)]
mod selector_tests;
mod terms;
use terms::supports_eval_term;

/// Выполнено ли условие `@supports` (css-conditional-3 §4).
///
/// Трёхзначная логика: неизвестная конструкция (`general-enclosed`) — не
/// ложь и не истина, а «неизвестно»; на верхнем уровне неизвестное и
/// невалидное равнозначны лжи. Поддержку декларации сообщает разборщик
/// свойства явно, в том числе для значений, совпадающих с начальным стилем.
pub(super) fn supports(condition: &str, vcx: ValueCx<'_>) -> bool {
    supports_in(condition, &SelectorCx::default(), vcx)
}

/// `supports` inside a stylesheet: `selector()` and `attr(ns|…)` see the
/// sheet's `@namespace` prologue.
pub(super) fn supports_in(condition: &str, cx: &SelectorCx, vcx: ValueCx<'_>) -> bool {
    matches!(
        supports_condition(condition.trim(), cx, vcx),
        Some(SupTri::True)
    )
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum SupTri {
    True,
    False,
    Unknown,
}

pub(super) fn sup_not(t: SupTri) -> SupTri {
    match t {
        SupTri::True => SupTri::False,
        SupTri::False => SupTri::True,
        SupTri::Unknown => SupTri::Unknown,
    }
}

/// `not <терм>` | `<терм> (and <терм>)*` | `<терм> (or <терм>)*` — уровни
/// не смешиваются: `a and b or c` недействительно целиком.
fn supports_condition(s: &str, cx: &SelectorCx, vcx: ValueCx<'_>) -> Option<SupTri> {
    let s = s.trim();
    // `not` — слово: слитное `not(` лексится функцией и уходит в терм.
    if let Some(rest) = s.strip_prefix("not")
        && rest.starts_with(char::is_whitespace)
    {
        let rest = rest.trim_start();
        let (term, tail) = supports_take_term(rest)?;
        if !tail.trim().is_empty() {
            return None;
        }
        return Some(sup_not(supports_eval_term(term, cx, vcx)));
    }
    let (term, mut rest) = supports_take_term(s)?;
    let mut acc = supports_eval_term(term, cx, vcx);
    let mut op: Option<&str> = None;
    loop {
        let r = rest.trim_start();
        if r.is_empty() {
            return Some(acc);
        }
        let word = if let Some(w) = r.strip_prefix("and") {
            if !w.starts_with(char::is_whitespace) {
                return None;
            }
            rest = w;
            "and"
        } else if let Some(w) = r.strip_prefix("or") {
            if !w.starts_with(char::is_whitespace) {
                return None;
            }
            rest = w;
            "or"
        } else {
            return None;
        };
        if let Some(prev) = op {
            if prev != word {
                return None;
            }
        } else {
            op = Some(word);
        }
        let (term, tail) = supports_take_term(rest.trim_start())?;
        let v = supports_eval_term(term, cx, vcx);
        acc = match (word, acc, v) {
            ("and", SupTri::True, SupTri::True) => SupTri::True,
            ("and", SupTri::False, _) | ("and", _, SupTri::False) => SupTri::False,
            ("and", ..) => SupTri::Unknown,
            ("or", SupTri::True, _) | ("or", _, SupTri::True) => SupTri::True,
            ("or", SupTri::False, SupTri::False) => SupTri::False,
            _ => SupTri::Unknown,
        };
        rest = tail;
    }
}

/// Один терм: скобочная группа либо функция `имя(...)`; возврат — тело
/// терма (со скобками функции внутри среза) и хвост после него.
fn supports_take_term(s: &str) -> Option<(&str, &str)> {
    let open = s.find('(')?;
    if open != 0 && !selector_tokens::ident(&s[..open]) {
        return None;
    }
    let close = open + find_matching(&s[open..])?;
    Some((&s[..=close], &s[close + 1..]))
}

/// A nested supports group starts with the complete at-keyword token.
pub(super) fn supports_group(prelude: &str) -> bool {
    prelude
        .get(..9)
        .is_some_and(|head| head.eq_ignore_ascii_case("@supports"))
        && prelude[9..].starts_with(|c: char| c == '(' || c.is_ascii_whitespace())
}
