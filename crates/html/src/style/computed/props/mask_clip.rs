//! Группа свойств `Computed::apply_one`: `mask*`, `clip`, `clip-path`,
//! `shape-*`. Здесь — `shape-outside`/`shape-margin`/`shape-image-threshold`,
//! устаревший `clip: rect(...)` (CSS 2.1), `mask-size`, `mask-mode`,
//! `mask-type`, `mask-composite`; слои маски, `clip-path` и базовые фигуры —
//! в подмодулях `mask_layers`, `clip_mask`, `basic_shape`.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::values::value::Len;

mod basic_shape;
mod clip_mask;
mod mask_layers;
pub(super) mod shape_value;
use shape_value::shape_valid;

impl Computed {
    pub(crate) fn apply_mask_clip(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        match key {
            // `will-change` (css-will-change-1 §2.1) разбирается ОДНОЙ армой
            // рядом с `backdrop-filter`: вторая арма того же ключа в этом
            // `match` недостижима.
            "shape-outside" => {
                let t = v.trim();
                if t != "none" && !shape_valid(t) && ImageSource::parse(t, vcx).is_none() {
                    return;
                }
                self.accept_declaration();
                self.shape_outside = (t != "none").then(|| t.to_string());
                self.shape_image = ImageSource::parse(t, vcx);
            }
            "shape-margin" => self.shape_margin = apply_parsed!(self, Len::parse(v.trim(), vcx)),
            "shape-image-threshold" => {
                self.shape_threshold = apply_parsed!(
                    self,
                    v.trim().parse::<f32>().ok().map(|t| t.clamp(0.0, 1.0))
                );
            }
            // Устаревшее `clip` (CSS 2.1): rect с запятыми или пробелами;
            // `auto` в позиции — соответствующий край коробки.
            "clip" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::CLIP;
                    return;
                }
                let raw = parsed_value!(self, shape_value::clip_rect(v, vcx));
                self.clip_len = raw;
                self.clip_rect = raw.map(|sides| {
                    sides.map(|side| match side {
                        Some(Len::Px(n)) => Some(n),
                        Some(
                            Len::Em(n)
                            | Len::Ex(n)
                            | Len::Ch(n)
                            | Len::Ic(n)
                            | Len::Lh(n)
                            | Len::Vh(n)
                            | Len::Vw(n),
                        ) if n == 0.0 => Some(0.0),
                        _ => None,
                    })
                });
            }
            "mask-size" | "-webkit-mask-size" => mask_size::apply(self, v, vcx),
            "mask-mode" => {
                if !matches!(v, "luminance" | "alpha" | "match-source") {
                    return;
                }
                self.accept_declaration();
                self.mask_luminance = Some(v.trim() == "luminance");
                self.mask_alpha_mode = Some(v.trim() == "alpha");
            }
            "mask-type" => {
                self.mask_type_alpha = apply_parsed!(
                    self,
                    match v {
                        "alpha" => Some(true),
                        "luminance" => Some(false),
                        _ => None,
                    }
                )
            }
            "mask-composite" | "-webkit-mask-composite" => {
                if v.split(',').any(|w| {
                    !matches!(
                        w.trim(),
                        "add"
                            | "subtract"
                            | "intersect"
                            | "exclude"
                            | "source-over"
                            | "source-in"
                            | "source-out"
                            | "source-atop"
                            | "destination-over"
                            | "destination-in"
                            | "destination-out"
                            | "destination-atop"
                            | "xor"
                            | "copy"
                            | "clear"
                    )
                }) {
                    return;
                }
                self.accept_declaration();
                self.mask_composite = Some(
                    v.split(',')
                        .map(|t| match t.trim() {
                            "subtract" => 1,
                            "intersect" => 2,
                            "exclude" => 3,
                            _ => 0,
                        })
                        .collect(),
                );
            }
            _ => self.apply_mask_layers(key, v, hit, vcx),
        }
    }
}
