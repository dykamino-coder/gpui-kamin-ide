//! Computed::apply_one: mask*, clip, clip-path, shape-*.

use crate::style::computed::*;
use crate::style::values::value::Len;

mod basic_shape;
mod clip_mask;
mod mask_layers;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_mask_clip(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // `will-change` (css-will-change-1 §2.1) разбирается ОДНОЙ армой
            // рядом с `backdrop-filter`: вторая арма того же ключа в этом
            // `match` недостижима.
            "shape-outside" => {
                let t = v.trim();
                if t != "none" {
                    self.shape_outside = Some(t.to_string());
                }
            }
            "shape-margin" => self.shape_margin = Len::parse(v.trim()),
            "shape-image-threshold" => {
                self.shape_threshold = v.trim().parse::<f32>().ok().map(|t| t.clamp(0.0, 1.0));
            }
            // Устаревшее `clip` (CSS 2.1): rect с запятыми или пробелами;
            // `auto` в позиции — соответствующий край коробки.
            "clip" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::CLIP;
                    return;
                }
                if let Some(rest) = v.trim().strip_prefix("rect(") {
                    let rest = rest.trim_end_matches(')');
                    // Разделители — ЛИБО три запятые, ЛИБО одни пробелы:
                    // смешанная запись невалидна, свойство игнорируется
                    // (clip-rect-comma-002..004).
                    let commas = rest.matches(',').count();
                    if commas != 0 && commas != 3 {
                        return;
                    }
                    let parts: Vec<&str> = rest
                        .split([',', ' '])
                        .map(str::trim)
                        .filter(|t| !t.is_empty())
                        .collect();
                    if parts.len() == 4 {
                        let side = |t: &str| match t {
                            "auto" => None,
                            // Нулевая длина есть ноль в любой единице, и
                            // `rect(-0em, …)` обязан обрезать, а не читаться
                            // как `auto` (`visufx/clip-076…102`). Кегель на
                            // этом шаге ещё не известен, поэтому ненулевые
                            // относительные единицы по-прежнему мимо.
                            _ => match Len::parse(t) {
                                Some(Len::Px(v)) => Some(v),
                                Some(
                                    Len::Em(v)
                                    | Len::Ex(v)
                                    | Len::Ch(v)
                                    | Len::Ic(v)
                                    | Len::Lh(v)
                                    | Len::Vh(v)
                                    | Len::Vw(v),
                                ) if v == 0.0 => Some(0.0),
                                _ => None,
                            },
                        };
                        self.clip_rect = Some([
                            side(parts[0]),
                            side(parts[1]),
                            side(parts[2]),
                            side(parts[3]),
                        ]);
                        let raw = |t: &str| match t {
                            "auto" => None,
                            _ => Len::parse(t),
                        };
                        self.clip_len =
                            Some([raw(parts[0]), raw(parts[1]), raw(parts[2]), raw(parts[3])]);
                    }
                }
            }
            "mask-size" | "-webkit-mask-size" => mask_size::apply(self, v),
            "mask-mode" => {
                self.mask_luminance = Some(v.trim() == "luminance");
                self.mask_alpha_mode = Some(v.trim() == "alpha");
            }
            "mask-type" => self.mask_type_alpha = Some(v.trim() == "alpha"),
            "mask-composite" | "-webkit-mask-composite" => {
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
            _ => self.apply_mask_layers(key, val, v, hit),
        }
    }
}
