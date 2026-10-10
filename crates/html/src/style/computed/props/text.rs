//! Computed::apply_one: color, line-height, text-align/indent/transform/wrap, white-space, word-*, hyphens, vertical-align, line-clamp, text-box*, writing modes.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};
mod vertical_clamp;
mod breaking;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_text(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // Неразборный цвет делает объявление недействительным (§4.2):
            // прежнее значение живёт, а не сменяется умолчанием. Пустой слот
            // у нас и означает «взять у родителя», поэтому `inherit` его
            // очищает (`color-174`).
            "color" => {
                self.color = if v == "inherit" {
                    None
                } else {
                    Color::parse(v).or(self.color)
                }
            }
            "line-height" => {
                // Голое число в line-height — множитель, а не пиксели, и
                // наследуется оно множителем: у потомка своя высота строки.
                //
                // Доля — наоборот: §10.8.1 «Computed value: for <length> and
                // <percentage> the absolute value», то есть `200%` считается
                // от СВОЕГО кегля и наследуется уже точками. У нас обе записи
                // давали `Len::Pct`, доля доживала до потомка и множилась на
                // его кегль (`c548-ln-ht-003` против зелёной `-004` — та же
                // разметка, разная запись). `Len::Em` сводится к точкам до
                // наследования, поэтому доля тегируется им.
                // `normal` — ЗАДАННОЕ значение, а не «не задано»: незаданное
                // поле у нас берётся от родителя, и `p { line-height: normal }`
                // молча наследовал `:root { line-height: 50px }` — документ
                // уезжал вниз на полулидинг (`rlh-unit-001`: зелёный квадрат
                // ниже эталона на 30 точек). Меткой служит `Len::Auto`: у всех
                // потребителей высоты строки уже есть для неё запасная ветка
                // «по метрикам шрифта» (`inline.rs:719`, `:1529`, `:2445`), а
                // `apply.rs:1618` на `Len::Auto` явно ничего не задаёт.
                if v.eq_ignore_ascii_case("normal") {
                    self.line_height = Some(Len::Auto);
                    return;
                }
                let parsed = match v.parse::<f32>() {
                    Ok(mult) if !v.ends_with("px") => Some(Len::Pct(mult)),
                    _ => match Len::parse(v) {
                        Some(Len::Pct(k)) if v.trim_end().ends_with('%') => Some(Len::Em(k)),
                        other => other,
                    },
                };
                // Отрицательная высота строки недействительна (§10.8.1):
                // объявление отбрасывается целиком, прежнее значение живёт.
                let neg = |l: &Len| {
                    matches!(
                        l,
                        Len::Px(v) | Len::Em(v) | Len::Pct(v) | Len::Ex(v) | Len::Ch(v)
                            if *v < 0.0
                    )
                };
                self.line_height = match parsed {
                    Some(l) if neg(&l) => self.line_height,
                    other => other,
                };
            }
            // `text-justify: none` запрещает выключку целиком: строка с
            // `text-align: justify` прижимается к началу, как `start`
            // (css-text-3 §7.3). Прочие значения различают, ЧТО растягивать —
            // пробелы или знаки; у нас растягиваются пробелы, и это поведение
            // `auto`/`inter-word`.
            "text-justify" => {
                if matches!(
                    v,
                    "none" | "auto" | "inter-word" | "inter-character" | "distribute" | "ruby"
                ) {
                    self.ruby_justify = Some(v == "ruby");
                    self.justify_chars = match v {
                        "inter-word" => Some(0),
                        "inter-character" | "distribute" => Some(2),
                        _ => Some(1),
                    };
                }
                self.no_justify = match v {
                    "none" => Some(true),
                    "auto" | "inter-word" | "inter-character" | "distribute" | "ruby" => {
                        Some(false)
                    }
                    _ => self.no_justify,
                };
            }
            "text-align" => {
                self.text_align = match v {
                    "center" => Some(TextAlign::Center),
                    "right" => Some(TextAlign::Right),
                    "left" => Some(TextAlign::Left),
                    "start" => Some(TextAlign::Start),
                    "end" => Some(TextAlign::End),
                    "justify" | "justify-all" => Some(TextAlign::Justify),
                    _ => self.text_align,
                };
                // `justify-all` — это выключка ВМЕСТЕ с последней строкой:
                // сокращение от `text-align: justify` + `text-align-last:
                // justify`.
                if v == "justify-all" {
                    self.text_align_last = Some(TextAlign::Justify);
                }
            }
            "text-align-last" => {
                self.text_align_last = match v {
                    "center" => Some(TextAlign::Center),
                    "right" => Some(TextAlign::Right),
                    "left" => Some(TextAlign::Left),
                    "start" => Some(TextAlign::Start),
                    "end" => Some(TextAlign::End),
                    "justify" => Some(TextAlign::Justify),
                    _ => self.text_align_last,
                }
            }
            "letter-spacing" => self.letter_spacing = Len::parse_spacing(v),
            "text-overflow" => {
                self.text_overflow_inherit = v.trim().eq_ignore_ascii_case("inherit");
                if self.text_overflow_inherit {
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
                self.ellipsis = Some(on);
                self.overflow_marker = marker;
            }
            // Долгая запись `white-space-collapse` (css-text-4 §3.1) трогает
            // ТОЛЬКО схлопывание; перенос (`nowrap`) остаётся за
            // `text-wrap-mode`. У `preserve-spaces` и `discard` своего пути в
            // сборке текста нет — не трогаем.
            "white-space-collapse" => match v {
                "collapse" => {
                    self.keep_spaces = Some(false);
                    self.preserve_newlines = Some(false);
                    self.break_after_spaces = Some(false);
                }
                "preserve" => {
                    self.keep_spaces = Some(true);
                    self.preserve_newlines = Some(true);
                    self.break_after_spaces = Some(false);
                }
                "preserve-breaks" => {
                    self.keep_spaces = Some(false);
                    self.preserve_newlines = Some(true);
                    self.break_after_spaces = Some(false);
                }
                "break-spaces" => {
                    self.keep_spaces = Some(true);
                    self.preserve_newlines = Some(true);
                    self.break_after_spaces = Some(true);
                }
                _ => {}
            },
            "white-space" => white_space::apply(self, v),
            "word-spacing" => self.word_spacing = Len::parse_spacing(v),
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
                        _ => continue,
                    }
                    any = true;
                }
                if any {
                    self.text_transform = Some(case.unwrap_or(TextTransform::None));
                    self.text_transform_flags = flags;
                }
            }
            _ => self.apply_text_breaking(key, val, v, hit),
        }
    }
}
