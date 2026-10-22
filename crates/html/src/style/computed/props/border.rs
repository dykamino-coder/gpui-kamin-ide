//! Группа свойств `Computed::apply_one`: `border*`, `corner*`, радиусы,
//! `outline*`, `border-spacing`/`border-collapse`, `empty-cells`,
//! `border-image`, `border-shape`. Длинные свойства рамки и контура из
//! реестра — в `longhands` (их сокращения раскрыты разбором); углы и
//! картинка рамки — в подмодулях.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::values::value::Len;

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
            "border-collapse" => self.border_collapse = accept_parsed!(self, Some(v == "collapse")),
            "empty-cells" => self.empty_cells_hide = accept_parsed!(self, Some(v.trim() == "hide")),
            "border-spacing" => {
                let (a, b) = axis_pair(v, vcx);
                if a.is_some() && b.is_some() {
                    self.accept_declaration();
                }
                self.border_spacing = Some((a, b));
            }
            // Сокращение `border-image` пока разбирает обработчик: его части
            // — длинные свойства реестра (сбрасывает их и `border`).
            "border-image" => self.set_border_image(key, v, vcx),
            _ => self.apply_border_corners(key, v, hit, vcx),
        }
    }
}

/// Пара значений «вдоль оси»: одно значение — обе грани, два — по порядку.
fn axis_pair(v: &str, vcx: ValueCx<'_>) -> (Option<Len>, Option<Len>) {
    let mut it = v.split_whitespace();
    let a = it.next().and_then(|s| Len::parse(s, vcx));
    let b = it.next().and_then(|s| Len::parse(s, vcx)).or(a);
    (a, b)
}
