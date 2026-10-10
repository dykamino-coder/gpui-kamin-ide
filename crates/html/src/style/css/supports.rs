//! @supports: условия, селекторы, трёхзначная логика.

use crate::style::css::*;

mod terms;
use terms::supports_eval_term;

/// Выполнено ли условие `@supports` (css-conditional-3 §4).
///
/// Трёхзначная логика: неизвестная конструкция (`general-enclosed`) — не
/// ложь и не истина, а «неизвестно»; на верхнем уровне неизвестное и
/// невалидное равнозначны лжи. Поддержка декларации проверяется ОРАКУЛОМ:
/// разобранное объявление применяется к чистому стилю — изменился, значит
/// свойство и значение наши (той же механикой живёт реестр покрытия).
pub(super) fn supports(condition: &str) -> bool {
    matches!(supports_condition(condition.trim()), Some(SupTri::True))
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
fn supports_condition(s: &str) -> Option<SupTri> {
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
        return Some(sup_not(supports_eval_term(term)));
    }
    let (term, mut rest) = supports_take_term(s)?;
    let mut acc = supports_eval_term(term);
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
        let v = supports_eval_term(term);
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
    let bytes = s.as_bytes();
    // Функция: идентификатор вплотную к скобке.
    let mut name_end = 0;
    while name_end < bytes.len()
        && (bytes[name_end].is_ascii_alphanumeric() || bytes[name_end] == b'-')
    {
        name_end += 1;
    }
    let open = if name_end < bytes.len() && bytes[name_end] == b'(' {
        name_end
    } else if bytes.first() == Some(&b'(') {
        0
    } else {
        return None;
    };
    let mut depth = 0usize;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&s[..i + 1], &s[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}
