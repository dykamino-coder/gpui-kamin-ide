//! Computed::apply_mask_clip, продолжение цепочки: mask-origin/clip, mask-repeat, mask-position, clip-path/mask. Ветви в исходном порядке.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_mask_layers(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
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
