//! Сведения о длинном свойстве реестра по его месту в таблице.

use super::{Grammar, LonghandId, Slot};
use crate::document::state::ValueCx;

impl LonghandId {
    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    /// Начальное значение (css-cascade-5 §7.3.1) — текстом той же
    /// грамматики, что и объявление.
    pub fn initial(self) -> &'static str {
        Self::INITIAL[self as usize]
    }

    /// Наследуется ли по умолчанию: решает `unset` (§7.3.3).
    pub fn inherited(self) -> bool {
        Self::INHERITED[self as usize]
    }

    pub fn grammar(self) -> Grammar {
        Self::GRAMMAR[self as usize]
    }

    pub fn slot(self) -> Slot {
        Self::SLOT[self as usize]
    }

    /// Годится ли значение (не общее слово и без подстановки) этому
    /// свойству: негодное объявление отбрасывается разбором (CSS Syntax
    /// §4.1.7 / css-cascade-5 §4) и в каскад не попадает.
    pub fn accepts(self, value: &str, vcx: ValueCx<'_>) -> bool {
        crate::style::computed::Computed::accepts_longhand(self, value, vcx)
    }
}
