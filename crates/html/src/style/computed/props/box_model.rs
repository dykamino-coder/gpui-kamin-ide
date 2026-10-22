//! Группа свойств `Computed::apply_one`: `width`/`height`, `min-*`/`max-*`,
//! логические размеры, `padding*`, `margin*`, `aspect-ratio`, `box-sizing`.
//! Здесь — размеры с `calc-size()`, ключевыми словами `stretch`/`fit-content(N)`
//! и пометками `inherit`, а также `padding` (отрицательное роняет объявление);
//! стороны и логические свойства — в подмодулях `sides`, `logical_layout`.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::values::value::Len;

mod logical_layout;
pub(super) mod sides;
use sides::{parsed_sides, side_length};

impl Computed {
    pub(crate) fn apply_box_model(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        if let Some(index) = [
            "width",
            "height",
            "min-width",
            "min-height",
            "max-width",
            "max-height",
        ]
        .iter()
        .position(|name| *name == key)
        {
            self.apply_physical_size(index, v, vcx);
            return;
        }
        match key {
            "box-sizing" => {
                self.border_box = apply_parsed!(
                    self,
                    match v.to_ascii_lowercase().as_str() {
                        "border-box" => Some(true),
                        "content-box" => Some(false),
                        _ => None,
                    }
                );
            }

            "padding" => {
                if v == "inherit" {
                    self.padding_inherit = true;
                    return;
                }
                let parsed = parsed_value!(self, parsed_sides(v, false, vcx));
                self.padding = parsed;
                self.padding_inherit = false;
                // Спор с логическими сторонами решает порядок объявлений.
                self.side_seq.padding = [self.decl_seq; 4];
            }
            _ => self.apply_box_model_sides(key, v, hit, vcx),
        }
    }
}
