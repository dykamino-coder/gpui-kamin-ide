//! Группа свойств `Computed::apply_one`: шрифт. Длинные свойства шрифта
//! реестра (`font-*`, `line-height`) — в подмодуле `longhands` с разбором
//! `values`/`variants`. `math-*` и `caret-color` — в `props::inherited`.

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
        *hit = false;
    }
}
