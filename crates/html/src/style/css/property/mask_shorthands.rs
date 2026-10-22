//! Сокращения маски: `mask` (css-masking-1 §7.9: `<mask-layer>#`; кроме
//! слоёв сбрасывает `mask-border`) и `mask-border` (§8.7). Части идут в любом
//! порядке и не больше раза; опущенная часть слоя получает начальное
//! значение.

use super::LonghandId;
use super::layer_shorthands::{initial_layer, split_position};
use super::layer_words::{Words, layer_list, layer_tokens, size_run};
use crate::document::state::ValueCx;

/// `<coord-box>` (css-masking-1 §7.4/§7.5).
fn coord_box(w: &str) -> bool {
    matches!(
        w,
        "content-box" | "padding-box" | "border-box" | "fill-box" | "stroke-box" | "view-box"
    )
}

/// Слой `mask`: тексты `mask-image, -position, -size, -repeat, -origin,
/// -clip, -composite, -mode`.
fn mask_layer(layer: &str, w: &Words<'_>) -> Option<[Option<String>; 8]> {
    let t = layer_tokens(layer);
    let mut out: [Option<String>; 8] = Default::default();
    let (mut boxes, mut no_clip) = (vec![], false);
    let mut i = 0;
    while i < t.len() {
        let word = t[i].as_str();
        let put = |slot: &mut Option<String>, v: String| slot.replace(v).is_none();
        if w.position(word) {
            let end = (i..t.len().min(i + 4))
                .take_while(|&j| w.position(&t[j]))
                .last()?
                + 1;
            let (x, y) = split_position(&t[i..end], false)?;
            if !put(&mut out[1], format!("{x} {y}")) {
                return None;
            }
            i = end;
            if t.get(i).is_some_and(|s| s == "/") {
                let (size, next) = size_run(&t, i + 1, w)?;
                out[2] = Some(size);
                i = next;
            }
            continue;
        }
        let ok = match word {
            "repeat-x" | "repeat-y" => put(&mut out[3], word.to_string()),
            "repeat" | "space" | "round" | "no-repeat" => {
                let second = t
                    .get(i + 1)
                    .filter(|n| matches!(n.as_str(), "repeat" | "space" | "round" | "no-repeat"));
                let text = match second {
                    Some(s) => format!("{word} {s}"),
                    None => word.to_string(),
                };
                i += usize::from(second.is_some());
                put(&mut out[3], text)
            }
            "no-clip" => !std::mem::replace(&mut no_clip, true),
            b if coord_box(b) => {
                boxes.push(b.to_string());
                boxes.len() <= 2
            }
            "add" | "subtract" | "intersect" | "exclude" => put(&mut out[6], word.to_string()),
            "alpha" | "luminance" | "match-source" => put(&mut out[7], word.to_string()),
            _ if w.image(word) => put(&mut out[0], word.to_string()),
            _ => false,
        };
        if !ok {
            return None;
        }
        i += 1;
    }
    match (boxes.as_slice(), no_clip) {
        ([], false) => {}
        ([], true) => out[5] = Some("no-clip".into()),
        ([one], false) => {
            out[4] = Some(one.clone());
            out[5] = Some(one.clone());
        }
        ([one], true) => {
            out[4] = Some(one.clone());
            out[5] = Some("no-clip".into());
        }
        ([origin, clip], false) => {
            out[4] = Some(origin.clone());
            out[5] = Some(clip.clone());
        }
        _ => return None,
    }
    Some(out)
}

/// Значения длинных свойств `mask` по слоям и начальные `mask-border-*`.
pub(super) fn mask(value: &str, vcx: Option<ValueCx<'_>>) -> Option<Vec<String>> {
    let w = Words { vcx };
    let ids = super::ShorthandId::Mask.longhands();
    let mut lists: Vec<Vec<String>> = vec![vec![]; 8];
    for layer in layer_list(value)? {
        for (k, part) in mask_layer(layer, &w)?.into_iter().enumerate() {
            lists[k].push(part.unwrap_or_else(|| initial_layer(ids[k])));
        }
    }
    let mut out: Vec<String> = lists.into_iter().map(|l| l.join(", ")).collect();
    out.extend(ids[8..].iter().map(|id| id.initial().to_string()));
    Some(out)
}

/// `mask-border`: `source || slice [/ width? [/ outset]?]? || repeat ||
/// mode` — тексты шести длинных свойств.
pub(super) fn mask_border(value: &str, vcx: Option<ValueCx<'_>>) -> Option<Vec<String>> {
    let w = Words { vcx };
    let t = layer_tokens(value);
    let mut out: [Option<String>; 6] = Default::default();
    let number = |s: &str| w.length(s) || s.parse::<f32>().is_ok();
    let mut i = 0;
    while i < t.len() {
        let word = t[i].as_str();
        let put = |slot: &mut Option<String>, v: String| slot.replace(v).is_none();
        let ok = if number(word) || word == "fill" {
            let mut end = i;
            while end < t.len() && (number(&t[end]) || t[end] == "fill") {
                end += 1;
            }
            let ok = put(&mut out[1], t[i..end].join(" "));
            i = end;
            let mut seg = 2;
            while ok && seg <= 3 && t.get(i).is_some_and(|s| s == "/") {
                let mut end = i + 1;
                while end < t.len() && (number(&t[end]) || t[end] == "auto") {
                    end += 1;
                }
                if end > i + 1 && !put(&mut out[seg], t[i + 1..end].join(" ")) {
                    return None;
                }
                if seg == 3 && end == i + 1 {
                    return None;
                }
                i = end;
                seg += 1;
            }
            if !ok {
                return None;
            }
            continue;
        } else {
            match word {
                "stretch" | "repeat" | "round" | "space" => {
                    let second = t
                        .get(i + 1)
                        .filter(|n| matches!(n.as_str(), "stretch" | "repeat" | "round" | "space"));
                    let text = match second {
                        Some(s) => format!("{word} {s}"),
                        None => word.to_string(),
                    };
                    i += usize::from(second.is_some());
                    put(&mut out[4], text)
                }
                "luminance" | "alpha" => put(&mut out[5], word.to_string()),
                _ if w.image(word) => put(&mut out[0], word.to_string()),
                _ => false,
            }
        };
        if !ok {
            return None;
        }
        i += 1;
    }
    let ids = super::ShorthandId::MaskBorder.longhands();
    Some(
        out.into_iter()
            .zip(ids)
            .map(|(part, id)| part.unwrap_or_else(|| id.initial().to_string()))
            .collect(),
    )
}

/// Длинные свойства `mask`: слои, затем сброс `mask-border`.
pub(super) const MASK: &[LonghandId] = &[
    LonghandId::MaskImage,
    LonghandId::MaskPosition,
    LonghandId::MaskSize,
    LonghandId::MaskRepeat,
    LonghandId::MaskOrigin,
    LonghandId::MaskClip,
    LonghandId::MaskComposite,
    LonghandId::MaskMode,
    LonghandId::MaskBorderSource,
    LonghandId::MaskBorderSlice,
    LonghandId::MaskBorderWidth,
    LonghandId::MaskBorderOutset,
    LonghandId::MaskBorderRepeat,
    LonghandId::MaskBorderMode,
];
