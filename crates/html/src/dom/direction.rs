//! Первый сильный знак содержимого для `dir="auto"` (HTML §3.2.6.4) и сжатие
//! пробелов в скобках для разбора директив счётчиков.
//!
//! `first_strong` ищет по тексту детей первый знак классов L/R/AL, пропуская
//! `bdi` и куски со своей стороной (`dir`, `style.rtl`); `None` — сильных
//! знаков нет, сторона остаётся унаследованной. `squeeze_parens` делает
//! `reversed( x )` одним словом, потому что разбор идёт по словам.

use super::Node;

/// Первый СИЛЬНЫЙ знак содержимого: `true` — справа налево, `None` — сильных
/// знаков нет вовсе и сторона остаётся унаследованной.
pub(super) fn first_strong(nodes: &[Node]) -> Option<bool> {
    use unicode_bidi::BidiClass::*;
    for node in nodes {
        match node {
            Node::Text(t) => {
                for ch in t.chars() {
                    match unicode_bidi::bidi_class(ch) {
                        L => return Some(false),
                        R | AL => return Some(true),
                        _ => {}
                    }
                }
            }
            Node::Element(e) => {
                // Внутри куска со СВОЕЙ стороной искать нечего: он сам себе
                // абзац для этого правила.
                // HTML §3.2.6.4 (auto directionality): `bdi` и элементы с
                // допустимым `dir` пропускаются целиком, даже без сильного знака.
                let own_dir = e.attr("dir").is_some_and(|v| {
                    matches!(v.to_ascii_lowercase().as_str(), "ltr" | "rtl" | "auto")
                });
                if e.style.rtl.is_some() || e.tag == "bdi" || own_dir {
                    continue;
                }
                if let Some(found) = first_strong(&e.children) {
                    return Some(found);
                }
            }
        }
    }
    None
}

/// Сжать пробелы внутри скобок: `reversed( x )` — одна запись значения,
/// а разбор идёт по словам.
pub(super) fn squeeze_parens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0i32;
    for ch in text.chars() {
        match ch {
            '(' => {
                depth += 1;
                out.push(ch);
            }
            ')' => {
                depth -= 1;
                out.push(ch);
            }
            c if c.is_whitespace() && depth > 0 => {}
            c => out.push(c),
        }
    }
    out
}
