//! Стадия вычисленных значений, фон: используемый цвет фона от `currentcolor` решается цветом
//! самого элемента — и у унаследованного `background-color: inherit` (css-color-4 §6.4).

use super::*;
use crate::style::css::LonghandId;

pub(super) fn inherit_background(own: &Computed, c: &mut Computed) {
    // `currentcolor` фона решается ЗДЕСЬ: только теперь известен цвет самого
    // элемента. Вычисленное значение остаётся словом — его унаследуют дети
    // (`background-color: inherit`) и решат своим цветом заново.
    if own.inherit_longhands.contains(LonghandId::BackgroundColor)
        || c.background_color
            .as_ref()
            .is_some_and(|value| value.depends_on_current())
    {
        let current = c.current_color();
        c.derive_background(Some(current));
    }
}
