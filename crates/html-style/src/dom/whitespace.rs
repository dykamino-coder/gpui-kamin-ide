//! Текстовые узлы только из пробелов в стилизованном дереве документа.

use crate::dom::Node;

pub fn is_blank(n: &Node) -> bool {
    matches!(n, Node::Text(t) if blank_text(t))
}

/// Пустой ли текстовый узел ПО CSS.
///
/// Схлопывается только `space`, `tab`, `CR`, `LF`. `str::trim` снимает весь
/// юникодный пробел, и узел из идеографических U+3000 (или неразрывных
/// U+00A0) считался пустым: строка из них пропадала целиком, а абзац рвался
/// там, где рваться не должен (`trailing-ideographic-space-017`).
pub fn blank_text(t: &str) -> bool {
    t.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}

/// Сохраняет ли CSS пробелы с возможностью переноса после каждого из них.
pub fn break_spaces(c: &crate::style::computed::Computed) -> bool {
    c.inh.text_flow.break_after_spaces == Some(true)
}

/// Сохраняет ли CSS последовательности пробелов.
pub fn keep_spaces(c: &crate::style::computed::Computed) -> bool {
    c.inh.text_flow.keep_spaces == Some(true)
}
