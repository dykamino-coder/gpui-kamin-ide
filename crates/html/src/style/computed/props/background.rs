//! Группа свойств `Computed::apply_one`: `background*`, `box-shadow`,
//! `object-fit`, `image-orientation`. Здесь `background-image` разбирается в
//! типизированный `ImageSource` (`ImageSource::parse`): `none` гасит картинку и
//! градиент; негодная запись сохраняет прежние поля; `light-dark()` выбирает вариант по
//! схеме. Сокращение и списки слоёв уходят в подмодули (`shorthand`,
//! `gradient_layer`, `layer_props`).

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod gradient_layer;
pub(super) mod layer_props;
mod list_values;
mod shorthand;

impl Computed {
    pub(crate) fn apply_background(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            // Сокращение несёт всё сразу: `background: #fff url(a.png) no-repeat`
            // — и цвет, и картинку, и режим повтора. Раньше побеждало что-то
            // одно, и картинка терялась при заданном цвете.
            "background" | "background-color" => self.apply_background_shorthand(key, v, vcx),
            "box-shadow" if has_font_units(v) => {
                if !box_shadow_valid(v, vcx) {
                    return;
                }
                self.shadow_raw = accept_parsed!(self, Some(v.to_string()))
            }
            "box-shadow" => {
                if v == "inherit" {
                    self.shadow_inherit = true;
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
                self.shadow_raw = None;
                let (inset, outer): (Vec<&str>, Vec<&str>) = crate::style::css::split_args(v)
                    .into_iter()
                    .partition(|one| one.contains("inset"));
                self.shadows = parse_shadows(&outer.join(","), vcx);
                self.inset_shadows = parse_shadows(&inset.join(",").replace("inset", " "), vcx);
            }
            "object-fit" => {
                self.object_fit = apply_parsed!(
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
            "image-orientation" => {
                self.image_orient_none = apply_parsed!(
                    self,
                    matches!(v, "none" | "from-image").then_some(v == "none")
                )
            }
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
                    self.inherit_bits |= inh::BG_IMAGE;
                    return;
                }
                // `none` ГАСИТ картинку (§14.2.1): ветки под него не было
                // вовсе, и заданный ранее адрес переживал отмену.
                if v.trim().eq_ignore_ascii_case("none") {
                    self.accept_declaration();
                    self.bg_image = None;
                    self.gradient = None;
                    self.gradient_raw = None;
                    self.gradient_current = None;
                } else if let Some(image) = ImageSource::parse(v, vcx) {
                    self.accept_declaration();
                    match image {
                        ImageSource::Gradient {
                            ref value,
                            repeating: false,
                        } => {
                            self.gradient = Some(*value.clone());
                            self.gradient_current = self
                                .gradient
                                .as_ref()
                                .filter(|g| !g.colors.is_empty())
                                .cloned();
                            self.gradient_raw = Some(image);
                        }
                        ImageSource::Invalid => {
                            self.bg_image = None;
                            self.gradient = None;
                            self.gradient_raw = None;
                            self.gradient_current = None;
                        }
                        other => {
                            if matches!(other, ImageSource::Color(_) | ImageSource::Fallback { .. })
                            {
                                self.gradient = None;
                                self.gradient_raw = None;
                                self.gradient_current = None;
                            }
                            self.bg_image = Some(other);
                        }
                    }
                }
            }

            _ => self.apply_background_layers(key, v, hit, vcx),
        }
    }
}
