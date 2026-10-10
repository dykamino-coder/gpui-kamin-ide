//! Computed::apply_one: mask*, clip, clip-path, shape-*.

use crate::style::computed::*;
use crate::style::values::value::Len;

mod basic_shape;
mod clip_mask;

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
            // SVG-коробки (css-masking-1 §7.10/7.11, `<geometry-box>`):
            // у элемента с CSS-коробкой fill-box = content-box, stroke-box и
            // view-box = border-box; у SVG-ребёнка их считает
            // `svg::masked_layers` (mask-clip-2, mask-origin-3).
            "mask-origin" | "-webkit-mask-origin" => {
                self.mask_origin = match v.trim() {
                    "padding-box" => Some(2),
                    "content-box" => Some(3),
                    "fill-box" => Some(4),
                    "stroke-box" => Some(5),
                    "view-box" => Some(6),
                    _ => Some(0),
                }
            }
            "mask-clip" | "-webkit-mask-clip" => {
                self.mask_clip = match v.trim() {
                    "padding-box" => Some(2),
                    "content-box" => Some(3),
                    "fill-box" => Some(4),
                    "stroke-box" => Some(5),
                    "view-box" => Some(6),
                    "no-clip" => Some(255),
                    _ => Some(0),
                }
            }
            "mask-repeat" | "-webkit-mask-repeat" => {
                // Пооосно (css-backgrounds §3.4): `repeat-x` = repeat по x,
                // одна плитка по y; два слова — оси по порядку.
                //
                // Запись — СПИСОК по слоям (css-masking-1 §7.6:
                // `<repeat-style>#`). Прежде строка резалась только по
                // пробелам, и запятая уезжала внутрь самого слова
                // (`"no-repeat,"` не равно `"no-repeat"`): весь список
                // `no-repeat, repeat` читался как `repeat` по обеим осям, и
                // плитка первого слоя мостила всю коробку
                // (mask-image-3b/3e 1.27, mask-position-5 1.25).
                let one = |layer: &str| {
                    let t: Vec<&str> = layer.split_whitespace().collect();
                    match t.as_slice() {
                        ["repeat-x"] => (false, true),
                        ["repeat-y"] => (true, false),
                        [a] => (*a == "no-repeat", *a == "no-repeat"),
                        [a, b] => (*a == "no-repeat", *b == "no-repeat"),
                        _ => (false, false),
                    }
                };
                let list: Vec<(bool, bool)> = crate::style::css::split_args(v)
                    .iter()
                    .map(|l| one(l))
                    .collect();
                self.mask_no_repeat = Some(list.first().copied().unwrap_or((false, false)));
                self.mask_repeat_list = (!list.is_empty()).then_some(list);
                let mode = |w: &str| match w {
                    "space" => 2u8,
                    "round" => 3,
                    _ => 0,
                };
                let modes: Vec<(u8, u8)> = crate::style::css::split_args(v)
                    .iter()
                    .map(|l| {
                        let t: Vec<&str> = l.split_whitespace().collect();
                        match t.as_slice() {
                            [a] => (mode(a), mode(a)),
                            [a, b] => (mode(a), mode(b)),
                            _ => (0, 0),
                        }
                    })
                    .collect();
                self.mask_repeat_modes =
                    modes.iter().any(|&(x, y)| x > 0 || y > 0).then_some(modes);
            }
            "mask-position" | "-webkit-mask-position" => {
                let word = |t: &str| match t {
                    "left" | "top" => Some(Len::Pct(0.0)),
                    "center" => Some(Len::Pct(0.5)),
                    "right" | "bottom" => Some(Len::Pct(1.0)),
                    _ => Len::parse(t),
                };
                // Запись — СПИСОК по слоям (css-masking-1 §7.7:
                // `<position>#`): `top, bottom` — своя точка у каждого слоя.
                let one = |layer: &str| -> Option<(Len, Len, bool, bool)> {
                    let toks: Vec<&str> = layer.split_whitespace().collect();
                    // Четырёхзначная запись — пары «край смещение»: `left 40%
                    // bottom 60%` (css-backgrounds-3 §3.6); от правого/нижнего
                    // края доля зеркалится.
                    if toks.len() == 4 {
                        let pair = |edge: &str, off: &str| -> Option<(Len, bool)> {
                            let l = Len::parse(off)?;
                            Some((l, matches!(edge, "right" | "bottom")))
                        };
                        let horiz = matches!(toks[0], "left" | "right");
                        let (xe, xo, ye, yo) = if horiz {
                            (toks[0], toks[1], toks[2], toks[3])
                        } else {
                            (toks[2], toks[3], toks[0], toks[1])
                        };
                        let ((x, fx), (y, fy)) = (pair(xe, xo)?, pair(ye, yo)?);
                        return Some((x, y, fx, fy));
                    }
                    let first = toks.first().and_then(|t| word(t))?;
                    // ОДИНОЧНОЕ слово осевое (css-backgrounds-3 §3.6):
                    // `top` — это `center top`, а не `top center`. Прежде оно
                    // уходило в ось X, и плитка вставала в (0, середина)
                    // вместо (середина, 0) — проба `target/probe/pmask-axis.html`
                    // против эталона корпуса даёт 1.71 против 0.03 у `center top`.
                    let (x, y) = match (toks.len(), toks.get(1).and_then(|t| word(t))) {
                        (_, Some(second)) => (first, second),
                        (1, None) if matches!(toks[0], "top" | "bottom") => (Len::Pct(0.5), first),
                        (_, None) => (first, Len::Pct(0.5)),
                    };
                    Some((x, y, false, false))
                };
                let list: Vec<(Len, Len, bool, bool)> = crate::style::css::split_args(v)
                    .iter()
                    .filter_map(|l| one(l))
                    .collect();
                if let Some((x, y, fx, fy)) = list.first().copied() {
                    self.mask_pos = Some((x, y));
                    self.mask_pos_far = (fx, fy);
                }
                self.mask_pos_list = (!list.is_empty()).then_some(list);
            }
            "clip-path" | "mask" | "mask-image" => self.apply_clip_mask(key, val, v, hit),
            _ => *hit = false,
        }
    }
}
