//! Полный разбор аргументов трансформаций перед изменением связанных полей.

use super::*;

pub(super) fn transform_value_valid(mut raw: &str, vcx: ValueCx<'_>) -> bool {
    if raw.is_empty() {
        return false;
    }
    while !raw.trim().is_empty() {
        let Some(open) = raw.find('(') else {
            return false;
        };
        let Some(end) = crate::style::css::block_end(raw, open, '(') else {
            return false;
        };
        let name = raw[..open].trim().to_ascii_lowercase();
        let args: Vec<_> = raw[open + 1..end].split(',').map(str::trim).collect();
        let valid_count = match name.as_str() {
            "matrix" => args.len() == 6,
            "matrix3d" => args.len() == 16,
            "translate" | "scale" | "skew" => (1..=2).contains(&args.len()),
            "translate3d" | "scale3d" => args.len() == 3,
            "rotate3d" => args.len() == 4,
            "translatex" | "translatey" | "translatez" | "scalex" | "scaley" | "scalez"
            | "rotate" | "rotatex" | "rotatey" | "rotatez" | "skewx" | "skewy" | "perspective" => {
                args.len() == 1
            }
            _ => false,
        };
        if !valid_count || args.iter().any(|a| a.is_empty()) {
            return false;
        }
        let number = |v: &str| v.parse::<f32>().ok().filter(|n| n.is_finite()).is_some();
        // css-transforms-2 §transform-functions: масштаб — `<number> | <percentage>`.
        let factor = |v: &str| number(v.strip_suffix('%').unwrap_or(v));
        // css-transforms-1 §transform-functions: `<angle> | <zero>` любой единицей и calc().
        let angle = |v: &str| crate::style::values::value::angle::degrees(v, vcx, true).is_some();
        let length = |v: &str, percent: bool| {
            Len::parse_mixed(v, vcx).is_some_and(|l| {
                !matches!(
                    l,
                    Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
                ) && (percent || !matches!(l, Len::Pct(_)))
            })
        };
        let valid = match name.as_str() {
            "matrix" | "matrix3d" => args.iter().all(|a| number(a)),
            "scale" | "scale3d" | "scalex" | "scaley" | "scalez" => args.iter().all(|a| factor(a)),
            "translate" | "translatex" | "translatey" => args.iter().all(|a| length(a, true)),
            "translate3d" => {
                length(args[0], true) && length(args[1], true) && length(args[2], false)
            }
            "translatez" => length(args[0], false),
            // css-transforms-2 §funcdef-perspective: `<length [0,∞]> | none`.
            "perspective" => {
                args[0] == "none"
                    || (length(args[0], false)
                        && !matches!(Len::parse(args[0], vcx), Some(Len::Px(d)) if d < 0.0))
            }
            "rotate3d" => args[..3].iter().all(|a| number(a)) && angle(args[3]),
            _ => args.iter().all(|a| angle(a)),
        };
        if !valid {
            return false;
        }
        raw = raw[end + 1..].trim_start();
    }
    true
}

pub(super) fn origin_valid(v: &str, vcx: ValueCx<'_>) -> bool {
    let words = split_outside_parens(v);
    if !(1..=3).contains(&words.len()) {
        return false;
    }
    let xy = words[..words.len().min(2)].join(" ");
    super::super::background::values::position_valid(&xy, vcx)
        && (words.len() < 3
            || Len::parse(&words[2], vcx).is_some_and(|l| {
                !matches!(
                    l,
                    Len::Pct(_) | Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
                )
            }))
}

/// css-transforms-2 §propdef-rotate: `<angle> | [x | y | z | <number>{3}] && <angle>`.
/// Возвращает угол в градусах и признак оси z, которую выражает плоский путь.
pub(super) fn rotate_value(v: &str, vcx: ValueCx<'_>) -> Option<(f32, bool)> {
    let words = split_outside_parens(v);
    let angle = |w: &String| crate::style::values::value::angle::degrees(w, vcx, false);
    let (degrees, axis) = match words.as_slice() {
        [a, rest @ ..] if angle(a).is_some() => (angle(a)?, rest),
        [rest @ .., a] if angle(a).is_some() => (angle(a)?, rest),
        _ => return None,
    };
    let flat = match axis {
        [] => true,
        [k] if matches!(k.as_str(), "x" | "y" | "z") => k == "z",
        [x, y, z] => {
            let n = [x, y, z].map(|n| n.parse::<f32>().ok().filter(|n| n.is_finite()));
            let [Some(x), Some(y), Some(z)] = n else {
                return None;
            };
            x == 0.0 && y == 0.0 && z > 0.0
        }
        _ => return None,
    };
    Some((degrees, flat))
}
