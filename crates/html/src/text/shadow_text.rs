//! Текст копии для тени или маски: пробелы схлопнуты так же, как в абзаце,
//! чтобы копия совпала с нарисованным текстом.

/// Схлопывание пробелов для тени — той же формы, что и в абзаце.
pub(crate) fn normalize_for_shadow(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_space = false;
    for ch in raw.chars() {
        // Схлопываются только четыре знака CSS: идеографический и неразрывный
        // пробелы — обычные знаки со своей шириной (см. `text::inline`).
        let is_space = matches!(ch, ' ' | '\t' | '\n' | '\r');
        if is_space {
            if !prev_space {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
        prev_space = is_space;
    }
    out
}
