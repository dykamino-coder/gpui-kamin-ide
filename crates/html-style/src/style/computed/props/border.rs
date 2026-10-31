//! Группа свойств `Computed::apply_one`: `border*`, `corner*`, радиусы,
//! `outline*`, `border-image`, `border-shape`. Длинные свойства рамки и контура из
//! реестра — в `longhands` (их сокращения раскрыты разбором); углы и
//! картинка рамки — в подмодулях.

use crate::style::computed::*;
use crate::values::ValueCx;

mod corners;
mod image;
mod longhands;
#[cfg(test)]
mod tests;
pub mod values;

impl Computed {
    pub fn apply_border(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        if self.apply_border_longhand(key, v, vcx) {
            return;
        }
        match key {
            // Сокращение `border-image` пока разбирает обработчик: его части
            // — длинные свойства реестра (сбрасывает их и `border`).
            "border-image" => self.set_border_image(key, v, vcx),
            _ => self.apply_border_corners(key, v, hit, vcx),
        }
    }
}
