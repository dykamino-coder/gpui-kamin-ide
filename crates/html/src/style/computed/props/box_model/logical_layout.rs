//! Computed::apply_box_model, хвост цепочки: логические размеры и поля, раскладочные свойства (aspect-ratio, frame-sizing). Ветви в исходном порядке после физических размеров и полей.

use super::*;

impl Computed {
    pub(super) fn apply_box_model_logical(&mut self, key: &str, v: &str, hit: &mut bool) {
        if self.declaration_parse != ParseResult::Inactive
            && matches!(
                key,
                "inline-size"
                    | "block-size"
                    | "min-inline-size"
                    | "min-block-size"
                    | "max-inline-size"
                    | "max-block-size"
            )
        {
            let mut parsed = None;
            assign_size(&mut parsed, v);
            self.note_parsed(&parsed);
            if v == "none" {
                self.accept_declaration();
            }
        }
        match key {
            // --- Логические свойства ---------------------------------------
            // Письмо у нас только слева направо и сверху вниз, поэтому
            // логические оси совпадают с физическими один в один.
            // Разбор — тот же, что у физических `width`/`height`
            // (`assign_size`): смесь «доля ± точки» в `calc()` доживает до
            // раскладки (css-logical-1 §4.1 — те же значения, что у
            // `width`). Прежде `Len::parse` роняло `calc(100% - 20px)` в
            // ничто (`css-break/table/table-row-paint-htb-ltr-ref`).
            "inline-size" => assign_size(&mut self.logical().inline_size, v),
            "block-size" => assign_size(&mut self.logical().block_size, v),
            "min-inline-size" => assign_size(&mut self.logical().min_inline, v),
            "min-block-size" => assign_size(&mut self.logical().min_block, v),
            "max-inline-size" => assign_size(&mut self.logical().max_inline, v),
            "max-block-size" => assign_size(&mut self.logical().max_block, v),
            "padding-inline" | "padding-block" | "margin-inline" | "margin-block"
            | "inset-inline" | "inset-block" => {
                let (a, b) = axis_pair(v);
                if a.is_some() && b.is_some() {
                    self.accept_declaration();
                }
                let block = key.ends_with("block");
                let which = key.split('-').next().unwrap_or("").to_string();
                let seq = self.decl_seq;
                let logical = self.logical();
                let target = match which.as_str() {
                    "padding" => &mut logical.padding,
                    "margin" => &mut logical.margin,
                    _ => &mut logical.inset,
                };
                if block {
                    target.block_start = a;
                    target.block_end = b;
                    target.seq[2] = seq;
                    target.seq[3] = seq;
                } else {
                    target.inline_start = a;
                    target.inline_end = b;
                    target.seq[0] = seq;
                    target.seq[1] = seq;
                }
            }
            "padding-inline-start"
            | "padding-inline-end"
            | "padding-block-start"
            | "padding-block-end"
            | "margin-inline-start"
            | "margin-inline-end"
            | "margin-block-start"
            | "margin-block-end"
            | "inset-inline-start"
            | "inset-inline-end"
            | "inset-block-start"
            | "inset-block-end" => {
                let seq = self.decl_seq;
                let parsed = record_parsed!(self, Len::parse(v));
                let logical = self.logical();
                let target = if key.starts_with("padding") {
                    &mut logical.padding
                } else if key.starts_with("margin") {
                    &mut logical.margin
                } else {
                    &mut logical.inset
                };
                let (slot, i) = if key.ends_with("inline-start") {
                    (&mut target.inline_start, 0)
                } else if key.ends_with("inline-end") {
                    (&mut target.inline_end, 1)
                } else if key.ends_with("block-start") {
                    (&mut target.block_start, 2)
                } else {
                    (&mut target.block_end, 3)
                };
                *slot = parsed;
                target.seq[i] = seq;
            }

            // --- Раскладка --------------------------------------------------
            "aspect-ratio" => {
                // `auto <ratio>` (css-sizing-4 §5.1): слово `auto` означает,
                // что у замещаемого ПРИРОДНОЕ соотношение сильнее заявленного,
                // а у остальных коробок действует заявленное. Прошлый заход
                // отбрасывал слово и отдавал отношение всем подряд — срез
                // css-sizing 264 -> 259 (+3/−8, `replaced-element-020/029/030`);
                // теперь слово живёт флагом, и отрисовка замещаемого его
                // учитывает (`image_with::ratio_of`).
                let body = v
                    .split_whitespace()
                    .filter(|t| !t.eq_ignore_ascii_case("auto"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let ratio = match body.split_once('/') {
                    Some((a, b)) => match (a.trim().parse::<f32>(), b.trim().parse::<f32>()) {
                        (Ok(a), Ok(b)) if b != 0.0 => Some(a / b),
                        _ => None,
                    },
                    None => body.trim().parse().ok(),
                }
                // Вырожденное отношение (ноль или бесконечность в любой части)
                // ведёт себя как `auto` (css-sizing-4 Overview.bs:533: «If the
                // <ratio> is degenerate, the property instead behaves as
                // auto»; css-values-4 Overview.bs:1665): `0/1` давал
                // соотношение 0, и коробка `width: 100px` получала бесконечную
                // высоту (`zero-or-infinity-001`).
                .filter(|r: &f32| r.is_finite() && *r > 0.0);
                self.note_parsed(&ratio);
                if v == "auto" {
                    self.accept_declaration();
                }
                if v.split_whitespace().any(|t| t.eq_ignore_ascii_case("auto")) {
                    self.aspect_ratio_auto = ratio;
                    self.aspect_ratio = None;
                } else {
                    self.aspect_ratio_auto = None;
                    self.aspect_ratio = ratio;
                }
            }
            "frame-sizing" => {
                self.frame_sizing_height = accept_parsed!(
                    self,
                    matches!(v.trim().to_ascii_lowercase().as_str(), "content-height")
                );
            }
            _ => *hit = false,
        }
    }
}
