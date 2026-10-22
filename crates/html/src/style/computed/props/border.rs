//! Группа свойств `Computed::apply_one`: `border*`, `corner*`, радиусы,
//! `outline*`, `border-spacing`/`border-collapse`, `empty-cells`,
//! `border-image`, `border-shape`. Длинные свойства рамки и контура из
//! реестра — в `longhands` (их сокращения раскрыты разбором); углы и
//! картинка рамки — в подмодулях.

use crate::document::state::ValueCx;
use crate::style::computed::*;

mod corners;
mod image;
mod longhands;
#[cfg(test)]
mod tests;
pub(crate) mod values;

impl Computed {
    pub(crate) fn apply_border(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        if self.apply_border_longhand(key, v, vcx) {
            return;
        }
        match key {
            "border-collapse" => {
                self.border_collapse = apply_parsed!(
                    self,
                    matches!(v, "collapse" | "separate").then_some(v == "collapse")
                )
            }
            "empty-cells" => {
                self.empty_cells_hide =
                    apply_parsed!(self, matches!(v, "hide" | "show").then_some(v == "hide"))
            }
            // `<length [0,∞]>{1,2}` (css-tables-3 §border-spacing): одно
            // значение — обе оси, два — по порядку.
            "border-spacing" => {
                let words = split_outside_parens(v);
                if !(1..=2).contains(&words.len()) {
                    return;
                }
                let pair = apply_parsed!(
                    self,
                    values::line_width(&words[0], vcx)
                        .zip(values::line_width(&words[words.len() - 1], vcx))
                );
                self.border_spacing = pair.map(|(a, b)| (Some(a), Some(b)));
            }
            // Сокращение `border-image` пока разбирает обработчик: его части
            // — длинные свойства реестра (сбрасывает их и `border`).
            "border-image" => self.set_border_image(key, v, vcx),
            _ => self.apply_border_corners(key, v, hit, vcx),
        }
    }
}
