//! Computed::apply_text, продолжение цепочки: text-indent, text-box*, hanging-punctuation, разрывы
//! слов и строк, text-fit. Ветви в исходном порядке; не совпавший ключ уходит в
//! apply_text_vertical_clamp.

use super::*;
mod grammar;
mod indent;
mod text_box;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_text_breaking(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        if !grammar::valid(key, v) {
            return;
        }
        match key {
            // Отступ первой строки. Кроме длины значение несёт до двух
            // ключевых слов (css-text-3 §7.1): `each-line` повторяет отступ
            // после КАЖДОГО жёсткого разрыва, `hanging` переворачивает выбор —
            // отступ получают все строки, КРОМЕ той, что получила бы его.
            // Порядок слов свободный, поэтому значение разбирается по словам.
            "text-indent" => indent::apply(self, v, vcx),
            "text-box-trim" | "text-box-edge" | "text-box" => text_box::apply(self, key, v),
            "hanging-punctuation" => {
                let mut h = Hanging::default();
                for word in v.split_ascii_whitespace() {
                    match word {
                        "first" => h.first = true,
                        "last" => h.last = true,
                        "force-end" => h.force_end = true,
                        "allow-end" => h.allow_end = true,
                        _ => {}
                    }
                }
                self.hanging = parsed_value!(self, Some((h != Hanging::default()).then_some(h)));
            }
            // Разрыв ВНУТРИ слова разрешают по-разному, и разница видна на
            // экране. `word-break: break-all` и `line-break: anywhere` рвут
            // слово всегда. А `overflow-wrap` — только когда слово иначе не
            // влезает: короткое сначала целиком уходит на следующую строку.
            "text-autospace" => {
                self.accept_declaration();
                // `normal` = оба разряда, `no-autospace` = ни одного.
                // Разряды могут стоять и по отдельности, и вместе.
                let (alpha, numeric) = match v {
                    "no-autospace" => (false, false),
                    "normal" | "auto" => (true, true),
                    other => (
                        other.contains("ideograph-alpha"),
                        other.contains("ideograph-numeric"),
                    ),
                };
                self.autospace_alpha = Some(alpha);
                self.autospace_numeric = Some(numeric);
            }
            "word-space-transform" => {
                self.accept_declaration();
                // Точка переноса показывается пробелом: обычным или
                // идеографическим (css-text-4). `none` и `auto-phrase`
                // не показывают ничего.
                // Значение из ДВУХ слов (`ideographic-space auto-phrase`,
                // css-text-4 §word-space-transform) сравнением целиком не
                // ловилось и падало в `none`. Ключевое слово ищем среди
                // разделённых пробелом кусков.
                // Явное `none` хранится нулевым знаком, а не `None`: свойство
                // наследуемое, и `wbr { word-space-transform: none }` внутри
                // `space` обязано отменить замену у своей точки переноса
                // (`word-space-transform-004`), а `None` значит «как у
                // родителя».
                self.word_space_char = Some(
                    v.split_whitespace()
                        .find_map(|w| match w {
                            "space" => Some(' '),
                            "ideographic-space" => Some('\u{3000}'),
                            _ => None,
                        })
                        .unwrap_or('\0'),
                );
            }
            "overflow-wrap" | "word-wrap" => {
                if !matches!(v, "normal" | "break-word" | "anywhere") {
                    return;
                }
                self.accept_declaration();
                self.break_word = Some(matches!(v, "break-word" | "anywhere"));
                // `anywhere` отличается от `break-word` ровно одним: он МЕНЯЕТ
                // размер по минимальному содержимому — слово рвётся и при его
                // подсчёте. `break-word` на этот размер не влияет
                // (css-text-3 §5.5), и на этой разнице построено целое
                // семейство тестов.
                self.wrap_anywhere = Some(v == "anywhere");
            }
            "word-break" => {
                css_wide_reset!(self, auto_phrase, break_anywhere, keep_all);
                if !matches!(
                    v,
                    "normal" | "break-all" | "keep-all" | "break-word" | "auto-phrase"
                ) {
                    return;
                }
                self.accept_declaration();
                self.auto_phrase = Some(v == "auto-phrase");
                // `break-word` — устаревший псевдоним, и по спецификации он
                // равен `overflow-wrap: anywhere`, а не `break-word`: разница
                // в том, что `anywhere` УЧИТЫВАЕТСЯ в размере по минимальному
                // содержимому. Пока стоял `break_word`, ячейка с
                // `max-width: 0` не сжималась до знака (`word-break-min-content-001`).
                if v == "break-word" {
                    self.break_word = Some(true);
                    self.wrap_anywhere = Some(true);
                    self.break_anywhere = Some(false);
                    self.keep_all = Some(false);
                } else {
                    self.break_anywhere = Some(v == "break-all");
                    self.keep_all = Some(v == "keep-all");
                }
            }
            "line-break" => {
                css_wide_reset!(
                    self,
                    break_anywhere,
                    break_anywhere_strict,
                    line_break_loose
                );
                if !matches!(v, "auto" | "loose" | "normal" | "strict" | "anywhere") {
                    return;
                }
                self.accept_declaration();
                self.break_anywhere = Some(v == "anywhere");
                self.break_anywhere_strict = Some(v == "anywhere");
                // `auto` у Blink ведёт себя строго (`LineBreakStrictness::
                // kDefault`), поэтому послабления — только у явных
                // `normal`/`loose`; `line-break-normal-011`, `-loose-*`.
                self.line_break_loose = Some(match v {
                    "normal" => 1,
                    "loose" => 2,
                    _ => 0,
                });
            }
            "hyphenate-limit-lines" => {
                if v == "no-limit" {
                    self.accept_declaration();
                    self.hyphen_limit_lines = Some(usize::MAX);
                } else if let Ok(limit) = v.parse::<usize>() {
                    self.accept_declaration();
                    self.hyphen_limit_lines = Some(limit);
                }
            }
            "hyphenate-limit-chars" => {
                if let Some(limits) = super::hyphen_limits::parse_hyphenate_limit_chars(v) {
                    self.accept_declaration();
                    self.hyphen_limit_chars = Some(limits);
                }
            }
            "hyphenate-character" => {
                self.accept_declaration();
                // Значение — строка в кавычках; `auto` значит «сам знак
                // переноса».
                self.hyphen_char = Some(if v == "auto" {
                    "\u{2010}".to_string()
                } else {
                    // Экранирования строки CSS (css-syntax-3 §4.3.7):
                    // `"\00a0\0640"` — два знака, а не текст
                    // (`hyphenate-character-005`).
                    crate::style::computed::parse_util::unescape_content(
                        v.trim_matches(['"', '\'']),
                    )
                });
            }
            "text-fit" => {
                self.accept_declaration();
                self.text_fit = (v != "none").then(|| {
                    let mut f = TextFit {
                        grow: false,
                        shrink: false,
                        per_line: false,
                        all: false,
                        target: None,
                    };
                    for word in v.split_whitespace() {
                        match word {
                            "grow" => f.grow = true,
                            "shrink" => f.shrink = true,
                            "consistent" => f.per_line = false,
                            "per-line" => f.per_line = true,
                            "per-line-all" => {
                                f.per_line = true;
                                f.all = true;
                            }
                            other => {
                                if let Some(pct) = other.strip_suffix('%')
                                    && let Ok(n) = pct.parse::<f32>()
                                {
                                    f.target = Some(n / 100.0);
                                }
                            }
                        }
                    }
                    f
                });
            }
            _ => self.apply_text_vertical_clamp(key, v, hit, vcx),
        }
    }
}
