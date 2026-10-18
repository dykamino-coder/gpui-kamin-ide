//! Computed::apply_text, хвост цепочки: vertical-align/baseline-shift, line-clamp, block-ellipsis, writing-mode, text-orientation, hyphens, tab-size. Ветви в исходном порядке после apply_text_breaking.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_text_vertical_clamp(
        &mut self,
        key: &str,
        val: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            // CSS Inline 3 §5.2.2: shared alignment grammar; center is middle.
            "vertical-align" | "baseline-shift" => {
                if Len::parse(v, vcx).is_some()
                    || matches!(
                        v,
                        "baseline"
                            | "sub"
                            | "super"
                            | "middle"
                            | "center"
                            | "top"
                            | "bottom"
                            | "text-top"
                            | "text-bottom"
                    )
                {
                    self.accept_declaration();
                }
                // Sub/super shift within the line. Font fractions approximate
                // normal line-height; percentage shifts retain their own slot.
                self.vertical_shift = match v {
                    "super" => Some(-1.0 / 3.0),
                    "sub" => Some(1.0 / 5.0),
                    other => match Len::parse(other, vcx) {
                        // Positive shifts move upward, opposite our y axis.
                        Some(Len::Pct(k)) => {
                            // Процент — доля `line-height`, и считается он
                            // позже: кладём в своё поле.
                            self.vertical_shift_pct = (k != 0.0).then_some(-k);
                            None
                        }
                        Some(Len::Em(k)) => (k != 0.0).then_some(-k),
                        _ => None,
                    },
                };
                // Font metrics arrive after inheritance; retain relative units.
                match Len::parse(v, vcx) {
                    // Точки не зависят ни от чего — сразу в поле.
                    Some(Len::Px(px)) => self.vertical_shift_px = (px != 0.0).then_some(-px),
                    // Единицы шрифта ждут набора: у строчного своих метрик
                    // обычно нет, они приходят наследованием уже после
                    // разбора, и здесь вышли бы от чужой гарнитуры.
                    Some(l @ (Len::Ex(_) | Len::Ch(_))) => self.vertical_shift_len = Some(l),
                    _ => {}
                }
                self.vertical_align_text = match v {
                    "text-top" => Some(true),
                    "text-bottom" => Some(false),
                    _ => None,
                };
                self.vertical_align = match v {
                    "middle" | "center" => Some(Align::Center),
                    "top" => Some(Align::Start),
                    "bottom" => Some(Align::End),
                    "baseline" => Some(Align::Baseline),
                    _ => None,
                }
            }
            // css-overflow-4 §5.1: `none | [<integer> || <'block-ellipsis'>]
            // -webkit-legacy?`; `auto` — срез по высоте контейнера. Строка
            // многоточия идёт маркером абзаца (`lines::marker_str`).
            "line-clamp" => {
                self.accept_declaration();
                self.line_clamp = None;
                self.clamp_auto = None;
                self.clamp_legacy = Some(false);
                self.clamp_mark = None;
                // Одна строка многоточия без числа — `max-lines: none` при
                // `continue: collapse` (§5.1: «Sets continue to collapse if
                // either or both values are specified»), то есть срез по
                // высоте, как у `auto` (`line-clamp-balance-009`).
                let mut ellipsis_given = false;
                let mut rest = v.trim();
                while !rest.is_empty() {
                    let quote = rest.as_bytes()[0];
                    let (word, tail) = if quote == b'"' || quote == b'\'' {
                        match rest[1..].find(quote as char) {
                            Some(end) => (&rest[..end + 2], &rest[end + 2..]),
                            None => (rest, ""),
                        }
                    } else {
                        match rest.find(char::is_whitespace) {
                            Some(end) => (&rest[..end], &rest[end..]),
                            None => (rest, ""),
                        }
                    };
                    rest = tail.trim_start();
                    if word.len() >= 2 && (word.starts_with('"') || word.starts_with('\'')) {
                        self.clamp_mark = Some(word[1..word.len() - 1].to_string());
                        ellipsis_given = true;
                    } else if word.eq_ignore_ascii_case("no-ellipsis") {
                        self.clamp_mark = Some(String::new());
                        ellipsis_given = true;
                    } else if word.eq_ignore_ascii_case("auto") {
                        self.clamp_auto = Some(true);
                    } else if let Ok(n) = word.parse::<u32>() {
                        self.line_clamp = Some(n);
                    }
                }
                if ellipsis_given && self.line_clamp.is_none() {
                    self.clamp_auto = Some(true);
                }
            }
            // Лонгхенд `block-ellipsis`: `no-ellipsis | auto | <string>`.
            "block-ellipsis" => {
                self.accept_declaration();
                let t = v.trim();
                self.clamp_mark = if t.eq_ignore_ascii_case("no-ellipsis") {
                    Some(String::new())
                } else if t.len() >= 2 && (t.starts_with('"') || t.starts_with('\'')) {
                    Some(collapse_forced_breaks(&unescape_content(
                        &t[1..t.len() - 1],
                    )))
                } else {
                    None
                };
            }
            // CSS Overflow 4 §5.2: collapse enables height clamping without
            // max-lines; auto undoes it. The initial block ellipsis is empty.
            "continue" => match v.trim().to_ascii_lowercase().as_str() {
                "collapse" => {
                    self.accept_declaration();
                    self.clamp_auto = Some(true);
                    self.clamp_legacy = Some(false);
                    if self.clamp_mark.is_none() {
                        self.clamp_mark = Some(String::new());
                    }
                }
                "auto" => {
                    self.accept_declaration();
                    self.clamp_auto = None;
                }
                _ => {}
            },
            "-webkit-line-clamp" => {
                self.line_clamp = record_parsed!(self, v.trim().parse().ok());
                self.clamp_auto = None;
                self.clamp_legacy = Some(true);
            }
            "-webkit-box-orient" => {
                self.webkit_box_vertical =
                    accept_parsed!(self, Some(v.eq_ignore_ascii_case("vertical")))
            }

            // --- Письмо и цветовые фильтры -------------------------------------
            "direction" | "unicode-bidi" => bidi_properties::apply(self, key, v),

            // --- Обтекание и направление письма --------------------------------
            // `initial-letter: normal | <size> [<sink> | drop | raise]`
            // (css-inline-3 §initial-letter). Число — высота буквицы в
            // строках (не меньше 1); целое — осадка; `raise` = 1; `drop` и
            // умолчание — осадка равна размеру, округлённому вниз. Порядок
            // слов свободный (`drop 3`). Кегль и строку буквицы считает
            // раскладка (`render::initial_letter_float`).
            "initial-letter" => {
                if v == "normal" {
                    self.accept_declaration();
                }
                self.initial_letter = None;
                if v != "normal" {
                    let mut size: Option<f32> = None;
                    let mut sink: Option<u32> = None;
                    for word in v.split_ascii_whitespace() {
                        match word {
                            "drop" => sink = Some(0),
                            "raise" => sink = Some(1),
                            w if size.is_none() => {
                                size = w.parse::<f32>().ok().filter(|n| *n >= 1.0);
                            }
                            w => sink = w.parse::<u32>().ok().filter(|n| *n >= 1),
                        }
                    }
                    if let Some(size) = size {
                        self.accept_declaration();
                        let sink = match sink {
                            Some(0) | None => (size.floor() as u32).max(1),
                            Some(n) => n,
                        };
                        self.initial_letter = Some((size, sink));
                    }
                }
            }
            // CSS Rhythm 1 §2: a step establishes an independent formatting
            // context even while rhythm layout itself is unimplemented.
            "block-step-size" => {
                if v == "none" || v == "auto" || Len::parse(v, vcx).is_some() {
                    self.accept_declaration();
                }
                if v != "none" && v != "auto" {
                    self.flow_root = Some(true);
                }
            }
            "text-orientation" => {
                self.accept_declaration();
                self.text_orientation =
                    Some(crate::style::computed::writing_mode::TextOrientation::parse(v));
            }
            "writing-mode" => {
                self.accept_declaration();
                self.writing_mode = Some(
                    crate::style::computed::writing_mode::WritingModeKw::parse(v),
                );
            }
            "text-combine-upright" => {
                let mut it = v.split_whitespace();
                self.combine_upright = record_parsed!(
                    self,
                    match it.next() {
                        Some("none") | None => None,
                        Some("all") => Some(0),
                        Some("digits") => Some(it.next().and_then(|n| n.parse().ok()).unwrap_or(2)),
                        _ => None,
                    }
                );
                if v == "none" {
                    self.accept_declaration();
                }
            }

            // --- Переносы и обрезка -------------------------------------------
            "hyphens" => {
                self.accept_declaration();
                self.hyphenate = Some(v != "none");
                // `manual` только УВАЖАЕТ расставленные знаки мягкого
                // переноса, сам их не ставит. Разделять обязательно: под одним
                // флагом `auto` и `manual` вели себя одинаково.
                self.hyphens_auto = Some(v == "auto");
            }
            "tab-size" => tab_size::apply(self, v, vcx),
            _ => *hit = false,
        }
    }
}
