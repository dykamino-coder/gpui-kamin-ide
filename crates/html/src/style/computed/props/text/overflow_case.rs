//! Computed::apply_text, продолжение цепочки: text-overflow, white-space(-collapse), word-spacing,
//! text-transform. Ветви в исходном порядке; не совпавший ключ уходит в apply_text_breaking.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_text_overflow_case(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            "text-overflow" => {
                if v.trim().eq_ignore_ascii_case("inherit") {
                    self.text_overflow_inherit = true;
                    return;
                }
                // css-overflow-4 §5: clip | ellipsis | <строка>, до двух
                // сторон. Наша обрезка — конец строки: берётся последнее
                // не-clip значение.
                let mut on = false;
                let mut marker = None;
                // Резка по пробелам ВНЕ кавычек: маркер-строка может
                // нести пробел; метка '\u{0}' отличает строку от ключевого слова.
                let mut tokens: Vec<String> = vec![];
                let mut cur = String::new();
                let mut quote: Option<char> = None;
                for ch in v.chars() {
                    match quote {
                        Some(q) if ch == q => quote = None,
                        Some(_) => cur.push(ch),
                        None if ch == '"' || ch == '\'' => {
                            quote = Some(ch);
                            if cur.is_empty() {
                                cur.push('\u{0}');
                            }
                        }
                        None if ch.is_whitespace() => {
                            if !cur.is_empty() {
                                tokens.push(std::mem::take(&mut cur));
                            }
                        }
                        None => cur.push(ch),
                    }
                }
                if !cur.is_empty() {
                    tokens.push(cur);
                }
                if quote.is_some()
                    || tokens.is_empty()
                    || tokens.len() > 2
                    || !tokens.iter().all(|t| {
                        t.starts_with('\u{0}') || matches!(t.as_str(), "clip" | "ellipsis")
                    })
                {
                    return;
                }
                self.text_overflow_inherit = false;
                for t in tokens {
                    if let Some(text) = t.strip_prefix('\u{0}') {
                        on = true;
                        // Строка объявления несёт экранирование (css-syntax
                        // §4.3.7: `\0A` — перевод строки, `\2026` — многоточие),
                        // а разрывы сегмента в ней преобразуются, как в тексте
                        // (css-text-3 §4.1.2; Blink `line_truncator.cc`
                        // `SuppressLineBreaks`): ряд принудительных разрывов —
                        // один пробел (`text-overflow-string-009…016`).
                        marker = Some(collapse_forced_breaks(&unescape_content(text)));
                    } else if t == "ellipsis" {
                        on = true;
                        marker = None;
                    }
                }
                self.ellipsis = accept_parsed!(self, Some(on));
                self.overflow_marker = marker;
            }
            // Долгая запись `white-space-collapse` (css-text-4 §3.1) трогает
            // ТОЛЬКО схлопывание; перенос (`nowrap`) остаётся за
            // `text-wrap-mode`. У `preserve-spaces` и `discard` своего пути в
            // сборке текста нет — не трогаем.
            "white-space-collapse" => match v {
                "collapse" => {
                    self.accept_declaration();
                    self.keep_spaces = Some(false);
                    self.preserve_newlines = Some(false);
                    self.break_after_spaces = Some(false);
                }
                "preserve" => {
                    self.accept_declaration();
                    self.keep_spaces = Some(true);
                    self.preserve_newlines = Some(true);
                    self.break_after_spaces = Some(false);
                }
                "preserve-breaks" => {
                    self.accept_declaration();
                    self.keep_spaces = Some(false);
                    self.preserve_newlines = Some(true);
                    self.break_after_spaces = Some(false);
                }
                "break-spaces" => {
                    self.accept_declaration();
                    self.keep_spaces = Some(true);
                    self.preserve_newlines = Some(true);
                    self.break_after_spaces = Some(true);
                }
                _ => {}
            },
            "white-space" => white_space::apply(self, v),
            "word-spacing" => {
                self.word_spacing = parsed_value!(
                    self,
                    Len::parse_spacing(v, vcx)
                        .map(Some)
                        .or_else(|| v.eq_ignore_ascii_case("inherit").then_some(None))
                )
            }
            "text-transform" => {
                // Свойство наследуемое: `inherit` очищает свой слот, иначе
                // прежнее объявление того же правила его переживало.
                if v == "inherit" {
                    self.text_transform = None;
                    self.text_transform_flags = 0;
                    return;
                }
                // Значений бывает несколько сразу (`uppercase full-width`):
                // регистр и добавки складываются, а не вытесняют друг друга —
                // прежде действовало только последнее слово
                // (`text-transform-multiple-001`).
                let mut case: Option<TextTransform> = None;
                let mut flags = 0u8;
                let mut any = false;
                for word in v.split_ascii_whitespace() {
                    match word.to_ascii_lowercase().as_str() {
                        "uppercase" => case = Some(TextTransform::Upper),
                        "lowercase" => case = Some(TextTransform::Lower),
                        "capitalize" => case = Some(TextTransform::Capitalize),
                        "full-width" | "fullwidth" => flags |= TT_FULL_WIDTH,
                        "full-size-kana" => flags |= TT_KANA,
                        "math-auto" => flags |= TT_MATH,
                        "none" => {}
                        _ => return,
                    }
                    any = true;
                }
                if any {
                    self.accept_declaration();
                    self.text_transform = Some(case.unwrap_or(TextTransform::None));
                    self.text_transform_flags = flags;
                }
            }
            _ => self.apply_text_breaking(key, v, hit, vcx),
        }
    }
}
