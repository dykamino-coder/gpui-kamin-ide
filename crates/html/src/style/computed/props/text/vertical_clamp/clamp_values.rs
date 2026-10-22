//! Чистый разбор составных значений обрезки и буквицы до изменения стиля.

type Clamp = (Option<u32>, Option<bool>, Option<String>);

pub(super) fn ellipsis_value(v: &str) -> Option<Option<String>> {
    match v {
        "auto" => Some(None),
        "no-ellipsis" => Some(Some(String::new())),
        _ if v.len() >= 2
            && (v.starts_with('"') && v.ends_with('"')
                || v.starts_with('\'') && v.ends_with('\'')) =>
        {
            Some(Some(super::collapse_forced_breaks(
                &super::unescape_content(&v[1..v.len() - 1]),
            )))
        }
        _ => None,
    }
}

/// css-overflow-4 §line-clamp (WPT `line-clamp-valid`, Blink `AlternativeLineClampShorthand`):
/// `none | [<integer [1,∞]> || auto || [ellipsis | no-ellipsis | <string>]] -webkit-legacy?`.
pub(super) fn clamp_value(v: &str) -> Option<Clamp> {
    if v == "none" {
        return Some((None, None, None));
    }
    let v = v.strip_suffix("-webkit-legacy").map_or(v, str::trim);
    // Строка — один токен даже с пробелами внутри (`2 " CUSTOM"`).
    let words = crate::style::css::split_top_level(v, ' ')
        .into_iter()
        .flat_map(|w| {
            if w.starts_with(['"', '\'']) {
                vec![w]
            } else {
                w.split_whitespace().collect()
            }
        })
        .filter(|w| !w.is_empty());
    let (mut lines, mut auto, mut mark) = (None, None, None);
    let mut ellipsis = false;
    for word in words {
        if let Ok(n) = word.parse::<u32>() {
            if n == 0 || lines.replace(n).is_some() {
                return None;
            }
        } else if word == "auto" {
            if auto.replace(true).is_some() {
                return None;
            }
        } else {
            if ellipsis {
                return None;
            }
            mark = match word {
                "ellipsis" => None,
                "auto" => return None,
                _ => ellipsis_value(word)?,
            };
            ellipsis = true;
        }
    }
    if lines.is_none() && auto.is_none() && !ellipsis {
        return None;
    }
    if ellipsis && lines.is_none() {
        auto = Some(true);
    }
    Some((lines, auto, mark))
}

pub(super) fn initial_letter(v: &str) -> Option<Option<(f32, u32)>> {
    if v == "normal" {
        return Some(None);
    }
    let (mut size, mut sink) = (None, None);
    for word in v.split_whitespace() {
        match word {
            "drop" if sink.is_none() => sink = Some(0),
            "raise" if sink.is_none() => sink = Some(1),
            w if size.is_none() => {
                size = Some(
                    w.parse::<f32>()
                        .ok()
                        .filter(|n| n.is_finite() && *n >= 1.0)?,
                )
            }
            w if sink.is_none() => sink = Some(w.parse::<u32>().ok().filter(|n| *n >= 1)?),
            _ => return None,
        }
    }
    let size = size?;
    let sink = sink
        .filter(|n| *n != 0)
        .unwrap_or((size.floor() as u32).max(1));
    Some(Some((size, sink)))
}
