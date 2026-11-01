//! Группа свойств `Computed::apply_one`: `background-*`, `box-shadow`,
//! `object-fit`. Сокращения `background` и
//! `background-position` раскрыты разбором (`style::css::property`), сюда
//! приходят длинные свойства. Значение свойства слоёв — список (§2.1):
//! верхний слой пишется в поля стиля, весь список — в `bg_lists` этого
//! свойства (`lists`). `background-image` разбирается в `ImageSource`:
//! `none` гасит картинку и градиент; `light-dark()` выбирает вариант по
//! схеме. Грамматики — `values`.

use crate::style::computed::*;
use crate::values::ValueCx;

mod layer_props;
mod lists;
pub mod values;

impl Computed {
    pub fn apply_background(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        if let Some(prop) = BgListProp::from_key(key)
            && self.apply_background_list(prop, v, vcx)
        {
            return;
        }
        match key {
            "background-color" => self.apply_background_color(v, vcx),
            "box-shadow" if has_font_units(v) => {
                if !box_shadow_valid(v, vcx) {
                    return;
                }
                self.reset_effects.shadow_raw = accept_parsed!(self, Some(v.to_string()))
            }
            "box-shadow" => {
                if v == "inherit" {
                    self.reset_effects.shadow_inherit = true;
                    return;
                }
                // `inset` в записи означает тень ВНУТРИ фигуры: раньше такая
                // запись просто не рисовалась.
                // Негодная запись ОТБРАСЫВАЕТСЯ целиком, прежняя тень живёт
                // (§7.1 `none | <shadow>#`; `box-shadow-invalid-001`).
                if !box_shadow_valid(v, vcx) {
                    return;
                }
                self.accept_declaration();
                self.reset_effects.shadow_raw = None;
                let (inset, outer): (Vec<&str>, Vec<&str>) = crate::style::css::split_args(v)
                    .into_iter()
                    .partition(|one| one.contains("inset"));
                self.reset_effects.shadows = parse_shadows(&outer.join(","), vcx);
                self.reset_effects.inset_shadows =
                    parse_shadows(&inset.join(",").replace("inset", " "), vcx);
            }
            "object-fit" => {
                self.reset_shapes.object_fit = apply_parsed!(
                    self,
                    matches!(v, "fill" | "contain" | "cover" | "none" | "scale-down")
                        .then(|| v.to_string())
                )
            }
            // `image-orientation` (css-images-3 §5.4): `from-image | none |
            // [<angle> || flip]`. Угол со `flip` спека сама помечает
            // необязательным и устаревшим («optional to implement and
            // deprecated»), и в корпусе его не просит ни один рефтест —
            // разбираем два ключевых слова.
            // `light-dark(<image>, <image>)` (css-color-5 §light-dark,
            // `image-light-dark`): вариант по схеме узла.
            "background-image"
                if v.get(..11)
                    .is_some_and(|h| h.eq_ignore_ascii_case("light-dark(")) =>
            {
                let inner = v[11..].trim_end();
                let args = crate::style::css::split_args(inner.strip_suffix(')').unwrap_or(inner));
                if let [light, dark] = args.as_slice() {
                    let dark_now = crate::style::values::value::dark_scheme(vcx);
                    let pick = if dark_now { dark.trim() } else { light.trim() };
                    self.apply_background(key, pick, hit, vcx);
                }
            }
            "background-image" => {
                if v == "inherit" {
                    return;
                }
                // `none` ГАСИТ картинку (§14.2.1): ветки под него не было
                // вовсе, и заданный ранее адрес переживал отмену.
                if v.trim().eq_ignore_ascii_case("none") {
                    self.accept_declaration();
                    self.reset_background.bg_image = None;
                    self.reset_background.gradient = None;
                    self.reset_background.gradient_raw = None;
                    self.reset_background.gradient_current = None;
                } else if let Some(image) = ImageSource::parse(v, vcx) {
                    self.accept_declaration();
                    match image {
                        ImageSource::Gradient {
                            ref value,
                            repeating: false,
                        } => {
                            self.reset_background.gradient = Some(*value.clone());
                            self.reset_background.gradient_current = self
                                .reset_background
                                .gradient
                                .as_ref()
                                .filter(|g| !g.colors.is_empty())
                                .cloned();
                            self.reset_background.gradient_raw = Some(image);
                        }
                        ImageSource::Invalid => {
                            self.reset_background.bg_image = None;
                            self.reset_background.gradient = None;
                            self.reset_background.gradient_raw = None;
                            self.reset_background.gradient_current = None;
                        }
                        other => {
                            if matches!(other, ImageSource::Color(_) | ImageSource::Fallback { .. })
                            {
                                self.reset_background.gradient = None;
                                self.reset_background.gradient_raw = None;
                                self.reset_background.gradient_current = None;
                            }
                            self.reset_background.bg_image = Some(other);
                        }
                    }
                }
            }

            _ => self.apply_background_layers(key, v, hit, vcx),
        }
    }
}
