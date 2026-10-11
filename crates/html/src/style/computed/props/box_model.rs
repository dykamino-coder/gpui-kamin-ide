//! Computed::apply_one: width/height/min/max, logical sizes, padding*, margin*, aspect-ratio, box-sizing.

use crate::style::computed::*;
use crate::style::values::value::Len;

mod logical_layout;
mod sides;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_box_model(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            "box-sizing" => self.border_box = Some(v == "border-box"),

            // Отрицательная длина делает объявление размера невалидным
            // (CSS 2.1 §10): `max-height: -1px` доезжал до раскладки и
            // схлопывал коробку в ноль. У `min-*` отрицательное поднимает
            // сама раскладка, но объявление всё равно отбрасывается.
            "width" | "height" | "min-width" | "min-height" if calc_size_arg(v).is_some() => {
                let i = match key {
                    "width" => 0,
                    "height" => 1,
                    "min-width" => 2,
                    _ => 3,
                };
                let slot = match i {
                    0 => &mut self.width,
                    1 => &mut self.height,
                    2 => &mut self.min_width,
                    _ => &mut self.min_height,
                };
                match calc_size_arg(v) {
                    Some(CalcSize::Fixed(px)) => {
                        *slot = Some(Len::Px(px));
                        self.calc_size[i] = None;
                    }
                    Some(CalcSize::Over(f)) => {
                        *slot = if i < 2 { Some(Len::Auto) } else { None };
                        self.calc_size[i] = Some(f);
                    }
                    None => {}
                }
            }
            "width" => {
                self.width_inherit = v == "inherit";
                self.stretch_size[0] = stretch_keyword(v);
                self.fit_arg[0] = fit_content_arg(v);
                self.calc_size[0] = None;
                assign_size(&mut self.width, v);
            }
            "height" => {
                self.height_inherit = v == "inherit";
                self.stretch_size[1] = stretch_keyword(v);
                self.calc_size[1] = None;
                assign_size(&mut self.height, v);
            }
            // Пределы не наследуются, но `inherit` берёт значение родителя
            // явно (§6.2.1). Без этой ветки `assign_size` стирал слот в
            // `None`, и `max-height: inherit` снимал предел вовсе.
            "min-width" => {
                self.minmax_inherit[0] = v == "inherit";
                self.stretch_size[2] = stretch_keyword(v);
                self.fit_arg[1] = fit_content_arg(v);
                self.calc_size[2] = None;
                assign_size(&mut self.min_width, v);
            }
            "min-height" => {
                self.minmax_inherit[1] = v == "inherit";
                self.stretch_size[4] = stretch_keyword(v);
                self.calc_size[3] = None;
                assign_size(&mut self.min_height, v);
            }
            "max-width" => {
                self.minmax_inherit[2] = v == "inherit";
                self.stretch_size[3] = stretch_keyword(v);
                self.fit_arg[2] = fit_content_arg(v);
                assign_size(&mut self.max_width, v);
            }
            "max-height" => {
                self.minmax_inherit[3] = v == "inherit";
                self.stretch_size[5] = stretch_keyword(v);
                assign_size(&mut self.max_height, v);
            }

            "padding" => {
                if v == "inherit" {
                    self.padding_inherit = true;
                    return;
                }
                let parsed = Sides::shorthand(v);
                let neg = |l: &Option<Len>| matches!(l, Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if *n < 0.0);
                if neg(&parsed.top)
                    || neg(&parsed.right)
                    || neg(&parsed.bottom)
                    || neg(&parsed.left)
                {
                    return;
                }
                self.padding = parsed;
                // Спор с логическими сторонами решает порядок объявлений.
                self.side_seq.padding = [self.decl_seq; 4];
            }
            _ => self.apply_box_model_sides(key, val, v, hit),
        }
    }
}
