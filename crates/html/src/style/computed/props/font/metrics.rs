//! Computed::apply_font, хвост цепочки: font-size-adjust, font-stretch, caret-color. Ветви в исходном порядке после font-size..font-variant.

use super::*;

impl Computed {
    pub(super) fn apply_font_metrics(&mut self, key: &str, v: &str, hit: &mut bool) {
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
                // CSS Fonts 4 §2.3: exact keyword percentages or a
                // nonnegative percentage, including percentage math.
                let pct = match v.to_ascii_lowercase().as_str() {
                    "ultra-condensed" => Some(50.0),
                    "extra-condensed" => Some(62.5),
                    "condensed" => Some(75.0),
                    "semi-condensed" => Some(87.5),
                    "normal" => Some(100.0),
                    "semi-expanded" => Some(112.5),
                    "expanded" => Some(125.0),
                    "extra-expanded" => Some(150.0),
                    "ultra-expanded" => Some(200.0),
                    other
                        if other.ends_with('%')
                            || ["calc(", "min(", "max(", "clamp("]
                                .iter()
                                .any(|f| other.starts_with(f)) =>
                    {
                        match Len::parse(other) {
                            Some(Len::Pct(p)) if p.is_finite() && p >= 0.0 => Some(p * 100.0),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some(pct) = pct {
                    self.font_stretch = Some(pct);
                }
            }
            "caret-color" => self.caret_color = Color::parse(v),
            _ => *hit = false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stretch_preserves_fractional_percentages_and_keywords() {
        for (value, expected) in [
            ("extra-condensed", 62.5),
            ("semi-condensed", 87.5),
            ("semi-expanded", 112.5),
            ("87.5%", 87.5),
            ("0%", 0.),
            ("calc(50% + 25%)", 75.),
        ] {
            let mut c = Computed::default();
            c.apply_decls(&crate::style::css::parse_decls(&format!(
                "font-stretch: {value}"
            )));
            assert_eq!(c.font_stretch, Some(expected), "{value}");
        }
    }

    #[test]
    fn invalid_stretch_preserves_previous_declaration() {
        for value in [
            "123",
            "0",
            "-5%",
            "100%%",
            "NaN%",
            "inf%",
            "10px",
            "stretch",
            "87.5% extra",
        ] {
            let mut c = Computed::default();
            c.apply_decls(&crate::style::css::parse_decls(&format!(
                "font-stretch: 75%; font-stretch: {value}"
            )));
            assert_eq!(c.font_stretch, Some(75.), "{value}");
        }
    }
}
