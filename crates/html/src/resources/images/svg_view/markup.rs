//! Точечные правки текста SVG по диапазонам исходника: граница открывающего тега с учётом
//! кавычек, экранирование значения атрибута и применение набора правок одним проходом.
//! Исходная разметка сохраняется побайтно везде, кроме правленых диапазонов.

use std::ops::Range;

/// Применить правки (диапазон исходника → новый текст); диапазоны не пересекаются.
pub(crate) fn apply(text: &str, mut edits: Vec<(Range<usize>, String)>) -> Vec<u8> {
    edits.sort_by_key(|(range, _)| range.start);
    let mut result = text.to_string();
    for (range, value) in edits.into_iter().rev() {
        result.replace_range(range, &value);
    }
    result.into_bytes()
}

/// Текст для значения атрибута или содержимого элемента XML.
pub(crate) fn xml_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Позиция `>` открывающего тега, начатого в `start`; `>` в кавычках значений не считается.
pub(crate) fn opening_end(text: &str, start: usize) -> Option<usize> {
    let mut quote = None;
    for (i, c) in text[start..].char_indices() {
        if quote == Some(c) {
            quote = None;
        } else if quote.is_none() {
            if c == '>' {
                return Some(start + i);
            }
            if c == '\'' || c == '"' {
                quote = Some(c);
            }
        }
    }
    None
}
