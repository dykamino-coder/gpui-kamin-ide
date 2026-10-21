//! Группа свойств `Computed::apply_one`: `width`/`height`, `min-*`/`max-*`,
//! `box-sizing`; стороны (`margin-*`, `padding-*`, вставки) и логические
//! стороны и размеры — в `sides`, `aspect-ratio`/`frame-sizing` — в `aspect`.
//! Сокращения сторон сюда не доходят: они раскрыты разбором (реестр
//! `style::css::property`). Значения размеров приходят проверенными
//! (`values`); здесь — `calc-size()`, ключевые слова `stretch`/`fit-content(N)`
//! и пометки `inherit`.

use crate::document::state::ValueCx;
use crate::style::computed::*;

mod aspect;
mod sides;
mod values;
pub(in crate::style::computed) use sides::side_slot;

impl Computed {
    pub(crate) fn apply_box_model(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        // Место физического размера в каскаде: спор с логическим размером
        // решает `resolve_logical` (css-logical-1 §4).
        if let Some(i) = [
            "width",
            "height",
            "min-width",
            "min-height",
            "max-width",
            "max-height",
        ]
        .iter()
        .position(|k| *k == key)
        {
            self.side_order.size[i] = self.decl_order;
        }
        if super::calc_size::apply(self, key, v, vcx) {
            return;
        }
        match key {
            "box-sizing" => self.border_box = accept_parsed!(self, Some(v == "border-box")),

            // Отрицательная длина делает объявление размера невалидным
            // (CSS 2.1 §10): `max-height: -1px` доезжал до раскладки и
            // схлопывал коробку в ноль. У `min-*` отрицательное поднимает
            // сама раскладка, но объявление всё равно отбрасывается.
            "width" => {
                self.width_inherit = v == "inherit";
                self.stretch_size[0] = stretch_keyword(v);
                self.fit_arg[0] = fit_content_arg(v, vcx);
                self.calc_size[0] = None;
                assign_size(&mut self.width, v, vcx);
            }
            "height" => {
                self.height_inherit = v == "inherit";
                self.stretch_size[1] = stretch_keyword(v);
                self.calc_size[1] = None;
                assign_size(&mut self.height, v, vcx);
            }
            // Пределы не наследуются, но `inherit` берёт значение родителя
            // явно (§6.2.1). Без этой ветки `assign_size` стирал слот в
            // `None`, и `max-height: inherit` снимал предел вовсе.
            "min-width" => {
                self.minmax_inherit[0] = v == "inherit";
                self.stretch_size[2] = stretch_keyword(v);
                self.fit_arg[1] = fit_content_arg(v, vcx);
                self.calc_size[2] = None;
                assign_size(&mut self.min_width, v, vcx);
            }
            "min-height" => {
                self.minmax_inherit[1] = v == "inherit";
                self.stretch_size[4] = stretch_keyword(v);
                self.calc_size[3] = None;
                assign_size(&mut self.min_height, v, vcx);
            }
            "max-width" => {
                self.minmax_inherit[2] = v == "inherit";
                self.stretch_size[3] = stretch_keyword(v);
                self.fit_arg[2] = fit_content_arg(v, vcx);
                assign_size(&mut self.max_width, v, vcx);
            }
            "max-height" => {
                self.minmax_inherit[3] = v == "inherit";
                self.stretch_size[5] = stretch_keyword(v);
                assign_size(&mut self.max_height, v, vcx);
            }
            _ => self.apply_box_model_sides(key, v, hit, vcx),
        }
    }
}
