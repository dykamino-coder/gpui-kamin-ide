//! Computed::apply_text, хвост цепочки: vertical-align/baseline-shift, `-webkit-box-orient`,
//! `unicode-bidi`, `initial-letter`, `block-step-size`.

use super::*;
use crate::document::state::ValueCx;

mod clamp_values;
use clamp_values::initial_letter;

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
            "-webkit-box-orient" => {
                if !matches!(v, "horizontal" | "vertical" | "inline-axis" | "block-axis") {
                    return;
                }
                self.webkit_box_vertical =
                    accept_parsed!(self, Some(v.eq_ignore_ascii_case("vertical")))
            }

            // --- Письмо и цветовые фильтры -------------------------------------
            "unicode-bidi" => bidi_properties::apply(self, v),

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
            _ => *hit = false,
        }
    }
}
