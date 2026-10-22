//! Сокращения слоёв фона: `background` (css-backgrounds-4 §background:
//! `<bg-layer>#? , <final-bg-layer>`) и `background-position` (раскладка
//! `<bg-position>` на `background-position-x/-y`, css-backgrounds-4 §3.6;
//! Blink и Stylo держат оси отдельными длинными свойствами). Части слоя идут
//! в любом порядке и не больше раза; опущенная часть слоя получает
//! начальное значение, поэтому у каждого длинного свойства столько же слоёв,
//! сколько у сокращения. Цвет — только в последнем слое.

use super::LonghandId;
use super::layer_words::{Words, layer_list, layer_tokens, size_run};
use crate::document::state::ValueCx;

/// Ось положения: `center | [left | right] <length-percentage>? |
/// <length-percentage>` (и вертикальные слова у второй оси).
fn axis_of(word: &str) -> Option<usize> {
    match word {
        "left" | "right" => Some(0),
        "top" | "bottom" => Some(1),
        _ => None,
    }
}

/// `<bg-position>` (css-backgrounds-3 §3.6) из слов `run` — тексты осей
/// `(x, y)`. Три слова допускает только `<bg-position>`; `<position>`
/// (`mask-position`, css-values-4 §9.1) — одно, два или четыре.
pub(crate) fn split_position(run: &[String], three: bool) -> Option<(String, String)> {
    let kw = |w: &str| matches!(w, "left" | "right" | "top" | "bottom" | "center");
    let center = || "center".to_string();
    match run {
        [a] if axis_of(a) == Some(1) => Some((center(), a.clone())),
        [a] => Some((a.clone(), center())),
        [a, b] if kw(a) && kw(b) => match (axis_of(a), axis_of(b)) {
            (Some(0) | None, Some(1) | None) => Some((a.clone(), b.clone())),
            (Some(1) | None, Some(0) | None) => Some((b.clone(), a.clone())),
            _ => None,
        },
        [a, b] => {
            let x_ok = !kw(a) || matches!(a.as_str(), "left" | "right" | "center");
            let y_ok = !kw(b) || matches!(b.as_str(), "top" | "bottom" | "center");
            (x_ok && y_ok).then(|| (a.clone(), b.clone()))
        }
        _ if run.len() == 4 || (three && run.len() == 3) => edge_offsets(run),
        _ => None,
    }
}

/// Форма «край смещение» (`right 10px top`, `left 10% bottom 2em`): две
/// группы, у `center` смещения нет, оси разные.
fn edge_offsets(run: &[String]) -> Option<(String, String)> {
    let kw = |w: &str| matches!(w, "left" | "right" | "top" | "bottom" | "center");
    let mut groups: Vec<(String, Option<String>)> = vec![];
    let mut i = 0;
    while i < run.len() {
        let edge = run[i].clone();
        if !kw(&edge) {
            return None;
        }
        let offset = run.get(i + 1).filter(|w| !kw(w)).cloned();
        if offset.is_some() && edge == "center" {
            return None;
        }
        i += 1 + usize::from(offset.is_some());
        groups.push((edge, offset));
    }
    let [(a, ao), (b, bo)] = <[_; 2]>::try_from(groups).ok()?;
    let text = |k: &str, o: &Option<String>| match o {
        Some(o) => format!("{k} {o}"),
        None => k.to_string(),
    };
    match (axis_of(&a), axis_of(&b)) {
        (Some(0) | None, Some(1) | None) if !(a == "center" && b == "center") => {
            Some((text(&a, &ao), text(&b, &bo)))
        }
        (Some(1) | None, Some(0) | None) => Some((text(&b, &bo), text(&a, &ao))),
        _ => None,
    }
}

/// Слой `background`: тексты длинных свойств в порядке
/// `ShorthandId::Background.longhands()` (цвет последним).
fn background_layer(layer: &str, last: bool, w: &Words<'_>) -> Option<[Option<String>; 9]> {
    let t = layer_tokens(layer);
    let mut out: [Option<String>; 9] = Default::default();
    let mut boxes: Vec<&str> = vec![];
    let mut i = 0;
    while i < t.len() {
        let word = t[i].as_str();
        let lower = word.to_ascii_lowercase();
        let put = |slot: &mut Option<String>, v: String| slot.replace(v).is_none();
        if w.position(word) {
            let end = (i..t.len().min(i + 4))
                .take_while(|&j| w.position(&t[j]))
                .last()?
                + 1;
            let (x, y) = split_position(&t[i..end], true)?;
            if !put(&mut out[1], x) || !put(&mut out[2], y) {
                return None;
            }
            i = end;
            if t.get(i).is_some_and(|s| s == "/") {
                let (size, next) = size_run(&t, i + 1, w)?;
                out[3] = Some(size);
                i = next;
            }
            continue;
        }
        let ok = match lower.as_str() {
            "/" => return None,
            "repeat-x" | "repeat-y" => put(&mut out[4], lower.clone()),
            "repeat" | "space" | "round" | "no-repeat" => {
                let second = t
                    .get(i + 1)
                    .filter(|n| matches!(n.as_str(), "repeat" | "space" | "round" | "no-repeat"));
                let text = match second {
                    Some(s) => format!("{lower} {s}"),
                    None => lower.clone(),
                };
                i += usize::from(second.is_some());
                put(&mut out[4], text)
            }
            "scroll" | "fixed" | "local" => put(&mut out[5], lower.clone()),
            "border-box" | "padding-box" | "content-box" | "text" | "border-area" => {
                boxes.push(word);
                boxes.len() <= 2
            }
            _ if w.image(word) => put(&mut out[0], word.to_string()),
            _ if last && w.color(word) => put(&mut out[8], word.to_string()),
            _ => false,
        };
        if !ok {
            return None;
        }
        i += 1;
    }
    let clip_only = |b: &str| matches!(b, "text" | "border-area");
    match boxes.as_slice() {
        [] => {}
        [one] if clip_only(one) => {
            out[6] = Some("border-box".into());
            out[7] = Some(one.to_string());
        }
        [one] => {
            out[6] = Some(one.to_string());
            out[7] = Some(one.to_string());
        }
        [origin, clip] if !clip_only(origin) => {
            out[6] = Some(origin.to_string());
            out[7] = Some(clip.to_string());
        }
        _ => return None,
    }
    Some(out)
}

/// Значения длинных свойств `background` по слоям, склеенные списками.
pub(super) fn background(value: &str, vcx: Option<ValueCx<'_>>) -> Option<Vec<String>> {
    let w = Words { vcx };
    let layers = layer_list(value)?;
    let ids = super::ShorthandId::Background.longhands();
    let mut lists: Vec<Vec<String>> = vec![vec![]; ids.len()];
    for (n, layer) in layers.iter().enumerate() {
        let parts = background_layer(layer, n + 1 == layers.len(), &w)?;
        for (k, part) in parts.into_iter().enumerate() {
            lists[k].push(part.unwrap_or_else(|| initial_layer(ids[k])));
        }
    }
    // Цвет у фона один: его даёт последний слой.
    let color = lists[8].pop()?;
    let mut out: Vec<String> = lists[..8].iter().map(|l| l.join(", ")).collect();
    out.push(color);
    Some(out)
}

/// `background-position` по слоям — тексты `background-position-x` и `-y`.
pub(super) fn background_position(value: &str, vcx: Option<ValueCx<'_>>) -> Option<Vec<String>> {
    let w = Words { vcx };
    let (mut xs, mut ys) = (vec![], vec![]);
    for layer in layer_list(value)? {
        let t = layer_tokens(layer);
        if t.is_empty() || t.iter().any(|s| !w.position(s)) {
            return None;
        }
        let (x, y) = split_position(&t, true)?;
        xs.push(x);
        ys.push(y);
    }
    Some(vec![xs.join(", "), ys.join(", ")])
}

/// Начальное значение одного слоя.
pub(super) fn initial_layer(id: LonghandId) -> String {
    match id {
        LonghandId::MaskPosition => "0% 0%".into(),
        _ => id.initial().to_string(),
    }
}
