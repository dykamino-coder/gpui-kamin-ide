//! Computed::apply_font, хвост цепочки: font-size-adjust, font-stretch, caret-color. Ветви в исходном порядке после font-size..font-variant.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_font_metrics(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            "font-size-adjust" => {
                // css-fonts-5 §font-size-adjust:
                //   none | [ ex-height | cap-height | ch-width | ic-width
                //          | ic-height ]? [ from-font | <number [0,∞]> ]
                // Негодная запись (доля, `auto`, отрицательное число) роняет
                // объявление целиком, прежнее значение живёт (§4.2;
                // `font-size-adjust-006/007/008`).
                let lower = v.to_ascii_lowercase();
                if lower == "inherit" {
                    self.font_size_adjust = None;
                    return;
                }
                if lower == "none" || lower == "initial" {
                    self.font_size_adjust = Some((u8::MAX, 0.0));
                    return;
                }
                let toks = split_outside_parens(&lower);
                let (metric, number) = match toks.as_slice() {
                    [n] => ("ex-height", n.as_str()),
                    [m, n] => (m.as_str(), n.as_str()),
                    _ => return,
                };
                let metric = match metric {
                    "ex-height" => 0,
                    "cap-height" => 1,
                    "ch-width" => 2,
                    "ic-width" => 3,
                    "ic-height" => 4,
                    _ => return,
                };
                let want = if number == "from-font" {
                    f32::NAN
                } else {
                    match crate::style::values::value::number(number) {
                        Some(k) if k >= 0.0 => k,
                        _ => return,
                    }
                };
                self.font_size_adjust = Some((metric, want));
            }
            "font-stretch" => {
                // Ключевые слова CSS — это проценты от обычной ширины.
                let pct = match v {
                    "ultra-condensed" => Some(50),
                    "extra-condensed" => Some(62),
                    "condensed" => Some(75),
                    "semi-condensed" => Some(87),
                    "normal" => Some(100),
                    "semi-expanded" => Some(112),
                    "expanded" => Some(125),
                    "extra-expanded" => Some(150),
                    "ultra-expanded" => Some(200),
                    other => other.trim_end_matches('%').parse::<i32>().ok(),
                };
                self.font_stretch = pct.map(|p| p as f32);
            }
            "caret-color" => self.caret_color = Color::parse(v),
            _ => *hit = false,
        }
    }
}
