//! Значение-список свойства слоёв фона (css-backgrounds-3 §2.1: «comma-
//! separated list of values … the first value represents the top layer»):
//! верхний слой разбирается обработчиком свойства в поля стиля, весь список
//! хранится типизированными слоями в `bg_lists` этого свойства
//! (`Computed::bg_layers` повторяет короткие списки до числа картинок).
//! Свойство пишет только свой список. `background-color` — один цвет.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::values::value::StyleColor;

impl Computed {
    /// Список слоёв свойства `prop`: `true` — значение-список разобрано
    /// здесь; одиночное значение разбирает обработчик свойства.
    pub(super) fn apply_background_list(
        &mut self,
        prop: BgListProp,
        v: &str,
        vcx: ValueCx<'_>,
    ) -> bool {
        if prop == BgListProp::BlendMode && !background_blend_valid(v) {
            return true;
        }
        // An invalid value changes nothing (CSS Syntax 4.1.7): block parsing
        // has already dropped it, and a direct call is checked by the same
        // grammar.
        if let Some(id) = crate::style::css::LonghandId::from_name(prop.key())
            && crate::style::css::CssWide::parse(v).is_none()
            && !super::values::accepts(id, v, vcx)
        {
            return true;
        }
        self.bg_lists.remove(prop);
        if top_level_comma(v).is_none() {
            return false;
        }
        // Верхний слой — обычным разбором; список кладётся ПОСЛЕ: разбор
        // слоя того же свойства его бы снял.
        let first = background_layers(v)[0].to_string();
        self.apply_one(prop.key(), &first, vcx);
        self.bg_lists.set(prop, v.to_string(), vcx);
        true
    }

    /// `background-color` (css-backgrounds-3 §3.2). Цвет от `currentcolor`
    /// остаётся вычисленным словом и решается при слиянии с цветом
    /// элемента (css-color-4 §4.6).
    pub(super) fn apply_background_color(&mut self, v: &str, vcx: ValueCx<'_>) {
        if v.trim().eq_ignore_ascii_case("inherit") {
            self.background_inherit = true;
            return;
        }
        if let Some(color) = StyleColor::parse_value(v, vcx) {
            self.accept_declaration();
            self.background_inherit = false;
            self.bg_explicit = true;
            // Начальное `transparent` хранится отсутствием цвета, как у
            // прочих длинных свойств фона: вычисленное значение то же.
            let initial = v.trim().eq_ignore_ascii_case("transparent");
            self.set_background_color((!initial).then_some(color));
        }
    }
}
