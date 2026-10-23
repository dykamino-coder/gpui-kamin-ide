//! Группа свойств `Computed::apply_one`: текст. Длинные свойства текста и
//! оформления реестра (`white-space-collapse`, `text-wrap-*`,
//! `text-emphasis-style/-color`, `text-decoration-*`) — в `longhands` с
//! разбором `values`; `text-overflow`, `vertical-align`, `initial-letter`,
//! `unicode-bidi` и прочие свойства вне реестра — в подмодулях. Наследуемые
//! свойства строк реестра — в `props::inherited`.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::css::{Grammar, LonghandId};

mod longhands;
mod overflow_case;
pub(crate) mod values;
mod vertical_clamp;

impl Computed {
    pub(crate) fn apply_text(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        if let Some(id) = LonghandId::from_name(key)
            .filter(|id| matches!(id.grammar(), Grammar::Text | Grammar::TextDecor))
        {
            return self.apply_text_longhand(id, v, vcx);
        }
        self.apply_text_overflow_case(key, v, hit, vcx)
    }
}
