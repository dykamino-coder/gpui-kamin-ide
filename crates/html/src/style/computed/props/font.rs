//! Computed::apply_one: font*, font-variant/feature/synthesis/size-adjust/stretch/kerning.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod features;
mod metrics;

impl Computed {
    pub(crate) fn apply_font(&mut self, key: &str, v: &str, hit: &mut bool) {
        match key {
            "math-style" => {
                if matches!(v, "normal" | "compact") {
                    self.math_compact = accept_parsed!(self, Some(v == "compact"));
                }
            }
            "math-shift" => {
                if matches!(v, "normal" | "compact") {
                    self.math_shift = accept_parsed!(self, Some(v == "compact"));
                }
            }
            "math-depth" => {
                if let Some(depth) = super::super::math_style::depth(v) {
                    self.math_depth = accept_parsed!(self, Some(depth));
                }
            }
            "font-size" => {
                if v == "math" {
                    self.accept_declaration();
                    self.math_font_size = true;
                    self.font_size = None;
                    return;
                }
                self.math_font_size = false;
                // Отрицательный кегль и неразборная запись делают объявление
                // НЕВАЛИДНЫМ (§4.2, §15.7): прежнее значение остаётся, а не
                // стирается в `None` (`c526-font-sz-003`: `-0.5in`).
                let neg = |l: &Len| {
                    matches!(
                        l,
                        Len::Px(v) | Len::Em(v) | Len::Pct(v) | Len::Ex(v) | Len::Ch(v)
                            if *v < 0.0
                    )
                };
                // Абсолютные и относительные СЛОВА кегля (CSS 2.1 §15.7,
                // css-fonts-4 §absolute-size). Раньше слово давало `None`, а
                // `None` в модели значит «не задано», то есть наследование.
                // Из-за этого `font-size: initial` (через `initial_value` —
                // `medium`) не сбрасывал кегль корня, и абзац
                // `percentage-rem-low` набирался четырьмя точками вместо
                // шестнадцати. Таблица — та же, что у Chrome при базовом 16.
                let lower = v.to_ascii_lowercase();
                let word = match lower.as_str() {
                    "xx-small" => Some(Len::Px(9.0)),
                    "x-small" => Some(Len::Px(10.0)),
                    "small" => Some(Len::Px(13.0)),
                    "medium" => Some(Len::Px(16.0)),
                    "large" => Some(Len::Px(18.0)),
                    "x-large" => Some(Len::Px(24.0)),
                    "xx-large" => Some(Len::Px(32.0)),
                    "xxx-large" => Some(Len::Px(48.0)),
                    // Относительные — доли РОДИТЕЛЬСКОГО кегля: их сводит к
                    // точкам `resolve_em`, как обычный `em`.
                    "smaller" => Some(Len::Em(5.0 / 6.0)),
                    "larger" => Some(Len::Em(1.2)),
                    _ => None,
                };
                if let Some(l) = word {
                    self.accept_declaration();
                    self.font_size = Some(l);
                    // `larger`/`smaller` шагают по ТАБЛИЦЕ (§15.7: «if the
                    // parent element has font size 'medium', then 'larger'
                    // will make … 'large'»), а не множат: шесть шагов вверх от
                    // `xx-small` (9 → 26.9) не сходились с шестью вниз от
                    // `xx-large` (32 → 10.7) (`font-size-121`). Кегль родителя
                    // известен в `inline::inherit`; `Len::Em` — запас вне таблицы.
                    self.font_size_step = match lower.as_str() {
                        "larger" => 1,
                        "smaller" => -1,
                        _ => 0,
                    };
                    return;
                }
                self.font_size = match Len::parse(v) {
                    Some(l) if neg(&l) => self.font_size,
                    other => {
                        self.font_size_step = 0;
                        record_parsed!(self, other)
                    }
                };
            }
            "font-weight" => font_weight::apply(self, v),
            "font-style" => {
                let mut parts = v.split_whitespace();
                let kind = parts.next().unwrap_or("");
                let angle = parts.next().map(crate::text::fonts::matching::angle);
                if !matches!(kind, "normal" | "italic" | "oblique")
                    || (kind != "oblique" && angle.is_some())
                    || angle == Some(None)
                    || parts.next().is_some()
                {
                    return;
                }
                self.font_oblique_angle = accept_parsed!(self, angle.flatten());
                // Retain the angle for face selection. The platform currently
                // synthesizes only the legacy bare oblique request.
                self.italic = Some(kind == "italic" || (kind == "oblique" && angle.is_none()));
                // `oblique <angle>` — тоже наклон, а не курсив (css-fonts-4
                // §font-style-prop): подбор лица различает их.
                self.oblique = Some(v.starts_with("oblique"));
            }
            "font-family" => font_family::apply(self, v),

            _ => self.apply_font_features(key, v, hit),
        }
    }
}
