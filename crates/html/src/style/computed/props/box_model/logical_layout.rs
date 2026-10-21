//! Computed::apply_box_model, хвост цепочки: логические размеры и поля, раскладочные свойства
//! (aspect-ratio, frame-sizing). Ветви в исходном порядке после физических размеров и полей.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_box_model_logical(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        let size_index = [
            "inline-size",
            "block-size",
            "min-inline-size",
            "min-block-size",
            "max-inline-size",
            "max-block-size",
        ]
        .iter()
        .position(|name| *name == key);
        if let Some(i) = size_index {
            let parsed = parsed_value!(
                self,
                super::super::calc_size::ParsedSize::parse(v, i >= 4, vcx)
            );
            let logical = self.logical();
            let slot = match i {
                0 => &mut logical.inline_size,
                1 => &mut logical.block_size,
                2 => &mut logical.min_inline,
                3 => &mut logical.min_block,
                4 => &mut logical.max_inline,
                _ => &mut logical.max_block,
            };
            assign_size(slot, parsed.length);
            return;
        }
        match key {
            "padding-inline" | "padding-block" | "margin-inline" | "margin-block"
            | "inset-inline" | "inset-block" => {
                let pair = apply_parsed!(self, axis_pair(v, !key.starts_with("padding"), vcx));
                let (a, b) = (pair.map(|p| p.0), pair.map(|p| p.1));
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
                let parsed = apply_parsed!(self, side_length(v, !key.starts_with("padding"), vcx));
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
                // а у остальных коробок действует заявленное. Отбрасывать
                // слово и отдавать отношение всем подряд нельзя
                // (`replaced-element-020/029/030`): слово живёт флагом, и
                // отрисовка замещаемого его учитывает (`image_with::ratio_of`).
                let (auto, ratio) = parsed_value!(self, aspect_ratio(v));
                if auto {
                    self.aspect_ratio_auto = ratio;
                    self.aspect_ratio = None;
                } else {
                    self.aspect_ratio_auto = None;
                    self.aspect_ratio = ratio;
                }
            }
            "frame-sizing" => {
                self.frame_sizing_height = parsed_value!(
                    self,
                    match v {
                        "content-height" => Some(true),
                        "fixed" => Some(false),
                        _ => None,
                    }
                );
            }
            _ => *hit = false,
        }
    }
}

fn aspect_ratio(v: &str) -> Option<(bool, Option<f32>)> {
    let mut auto = false;
    let mut body = Vec::new();
    for token in v.split_whitespace() {
        if token.eq_ignore_ascii_case("auto") {
            if auto {
                return None;
            }
            auto = true;
        } else {
            body.push(token);
        }
    }
    if body.is_empty() {
        return auto.then_some((true, None));
    }
    let body = body.join(" ");
    let (a, b) = body.split_once('/').unwrap_or((&body, "1"));
    let a: f32 = a.trim().parse().ok()?;
    let b: f32 = b.trim().parse().ok()?;
    if a < 0.0 || b < 0.0 {
        return None;
    }
    let ratio = a / b;
    Some((auto, (ratio.is_finite() && ratio > 0.0).then_some(ratio)))
}
