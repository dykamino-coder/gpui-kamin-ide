//! Computed::apply_text, хвост цепочки: vertical-align/baseline-shift, line-clamp, block-ellipsis,
//! writing-mode, text-orientation, hyphens, tab-size. Ветви в исходном порядке после
//! apply_text_breaking.

use super::*;
use crate::document::state::ValueCx;

mod clamp_values;
use clamp_values::{clamp_value, ellipsis_value, initial_letter};

impl Computed {
    pub(super) fn apply_text_vertical_clamp(
        &mut self,
        key: &str,
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
                } else {
                    return;
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
                let (lines, auto, mark) = parsed_value!(self, clamp_value(v));
                self.line_clamp = lines;
                self.clamp_auto = auto;
                self.clamp_legacy = Some(v.ends_with("-webkit-legacy"));
                self.clamp_mark = mark;
            }

            // Лонгхенд `block-ellipsis`: `no-ellipsis | auto | <string>`.
            "block-ellipsis" => {
                self.clamp_mark = parsed_value!(self, ellipsis_value(v));
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
                self.line_clamp = parsed_value!(
                    self,
                    if v == "none" {
                        Some(None)
                    } else {
                        v.parse::<u32>().ok().filter(|n| *n > 0).map(Some)
                    }
                );
                self.clamp_auto = None;
                self.clamp_legacy = Some(true);
            }
            "-webkit-box-orient" => {
                if !matches!(v, "horizontal" | "vertical" | "inline-axis" | "block-axis") {
                    return;
                }
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
                self.initial_letter = parsed_value!(self, initial_letter(v));
            }

            // css-rhythm-1 §2: значение кроме `none`/`auto` создаёт независимый
            // контекст форматирования (`flow_root`), хотя сам шаг ритма не
            // считается (`block-step-size-establishes-*`).
            "block-step-size" => {
                if v == "none" || v == "auto" || Len::parse(v, vcx).is_some() {
                    self.accept_declaration();
                } else {
                    return;
                }
                if v != "none" && v != "auto" {
                    self.flow_root = Some(true);
                }
            }
            "text-orientation" => {
                if !matches!(
                    v,
                    "mixed" | "upright" | "sideways" | "sideways-right" | "use-glyph-orientation"
                ) {
                    return;
                }
                self.accept_declaration();
                self.text_orientation =
                    Some(crate::style::computed::writing_mode::TextOrientation::parse(v));
            }
            "writing-mode" => {
                if !matches!(
                    v,
                    "horizontal-tb"
                        | "vertical-rl"
                        | "vertical-lr"
                        | "sideways-rl"
                        | "sideways-lr"
                        | "lr"
                        | "lr-tb"
                        | "rl"
                        | "rl-tb"
                        | "tb"
                        | "tb-rl"
                ) {
                    return;
                }
                self.accept_declaration();
                self.writing_mode = Some(
                    crate::style::computed::writing_mode::WritingModeKw::parse(v),
                );
            }
            "text-combine-upright" => {
                let words: Vec<_> = v.split_whitespace().collect();
                self.combine_upright = parsed_value!(
                    self,
                    match words.as_slice() {
                        ["none"] => Some(None),
                        ["all"] => Some(Some(0)),
                        ["digits"] => Some(Some(2)),
                        ["digits", n] => n
                            .parse::<u8>()
                            .ok()
                            .filter(|n| (2..=4).contains(n))
                            .map(Some),
                        _ => None,
                    }
                );
            }

            // --- Переносы и обрезка -------------------------------------------
            "hyphens" => {
                if !matches!(v, "none" | "manual" | "auto") {
                    return;
                }
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
