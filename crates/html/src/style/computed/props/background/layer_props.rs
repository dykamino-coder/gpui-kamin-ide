//! Computed::apply_background, хвост цепочки: верхний слой длинных свойств
//! фона (`background-repeat`, `-origin`, `-clip`, `-size`, `-position-x/-y`,
//! `-attachment`, `-blend-mode`) и `object-position`/`object-view-box`.
//! Разбор — функции `values` (те же, что проверяют объявление); `inherit`
//! ставит разряд `inherit_bits`, значение его снимает. Начальное значение
//! хранится отсутствием значения: вычисленное значение у них одно.

use super::values::{attachment, bg_size, clip_box, origin_box, repeat_style};
use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_background_layers(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        let bit = match key {
            "background-repeat" => inh::BG_REPEAT,
            "background-origin" => inh::BG_ORIGIN,
            "background-clip" | "-webkit-background-clip" => inh::BG_CLIP,
            "background-size" => inh::BG_SIZE,
            "background-position-x" => inh::BG_POS,
            "background-position-y" => inh::BG_POS_Y,
            "background-attachment" => inh::BG_ATTACH,
            _ => 0,
        };
        if bit != 0 {
            if v.trim().eq_ignore_ascii_case("inherit") {
                self.inherit_bits |= bit;
                return;
            }
            self.inherit_bits &= !bit;
        }
        match key {
            "background-blend-mode" => {
                if let Some(mode) = BlendMode::parse(v) {
                    self.background_blend = accept_parsed!(self, Some(mode));
                } else if matches!(v, "initial" | "unset" | "revert" | "revert-layer") {
                    self.background_blend = None;
                }
            }
            // Запись бывает и поосевой: `repeat space`, `round no-repeat`.
            "background-repeat" => {
                if let Some(repeat) = record_parsed!(self, repeat_style(v)) {
                    self.bg_repeat = (repeat != BgRepeat::Axes(Tiling::Repeat, Tiling::Repeat))
                        .then_some(repeat);
                }
            }
            // Откуда отсчитывается картинка: умолчание — внутренний край
            // рамки (`padding-box`).
            "background-origin" => {
                if let Some(origin) = record_parsed!(self, origin_box(v)) {
                    self.bg_origin = origin;
                }
            }
            // Область покраски фона: умолчание — `border-box`.
            "background-clip" | "-webkit-background-clip" => {
                if let Some(clip) = record_parsed!(self, clip_box(v)) {
                    self.bg_clip = clip;
                }
            }
            // Отрицательная длина делает объявление негодным (§3.9).
            // Процентная смесь `calc(50px + 50%)` доживает до растра.
            "background-size" => {
                if let Some(size) = record_parsed!(self, bg_size(v, vcx)) {
                    self.bg_size = size;
                }
            }
            // Оси положения (css-backgrounds-4 §3.6): своя ось, другая не
            // трогается.
            "background-position-x" | "background-position-y" => {
                self.set_background_position_axis(usize::from(key.ends_with("-y")), v.trim(), vcx);
            }
            // `fixed` привязывает плитку к ОБЛАСТИ ПРОСМОТРА: считается она от
            // окна, а красится всё равно только внутри коробки (§3.5).
            "background-attachment" => {
                if let Some(fixed) = record_parsed!(self, attachment(v)) {
                    self.bg_fixed = fixed.then_some(true);
                }
            }
            // `object-position` — та же грамматика, но для замещаемого
            // содержимого (css-images-3 §5.2).
            "object-view-box" => {
                self.object_view_box = record_parsed!(self, parse_view_box(v, vcx));
                if v == "none" {
                    self.accept_declaration();
                }
            }
            "object-position" => {
                self.object_position = accept_parsed!(self, Some(parse_pos_words(v, vcx)));
            }
            _ => *hit = false,
        }
    }
}
