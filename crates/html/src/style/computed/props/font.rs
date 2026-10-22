//! Группа свойств `Computed::apply_one`: шрифт. Длинные свойства шрифта
//! реестра (`font-*`, `line-height`) — в подмодуле `longhands` с разбором
//! `values`/`variants`; здесь же `math-style`, `math-shift`, `math-depth`
//! (MathML Core §3.1-3.2) и `caret-color` (css-ui-4 §6.2.1).

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::css::{Grammar, LonghandId};

mod longhands;
mod size;
#[cfg(test)]
mod tests;
pub(crate) mod values;
mod variants;

impl Computed {
    pub(crate) fn apply_font(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        if let Some(id) = LonghandId::from_name(key).filter(|id| id.grammar() == Grammar::Font) {
            return self.apply_font_longhand(id, v, vcx);
        }
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
            "caret-color" => {
                if let Some(value) = super::effects::values::inherited_color_or_auto(self, v, vcx) {
                    self.accept_declaration();
                    self.caret_color = value;
                }
            }
            _ => *hit = false,
        }
    }
}
