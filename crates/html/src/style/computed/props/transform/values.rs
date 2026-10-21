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
        let angle = |v: &str| crate::text::fonts::matching::angle(v).is_some();
        let length = |v: &str| {
            Len::parse_mixed(v, vcx).is_some_and(|l| {
                !matches!(
                    l,
                    Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
                )
            })
        };
        let valid = match name.as_str() {
            "matrix" | "matrix3d" | "scale3d" | "scalex" | "scaley" | "scalez" => {
                args.iter().all(|a| number(a))
            }
            "scale" => args
                .iter()
                .all(|a| number(a.strip_suffix('%').unwrap_or(a))),
            "translate" | "translate3d" | "translatex" | "translatey" | "translatez"
            | "perspective" => args.iter().all(|a| length(a)),
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
    super::super::background::layer_props::position_valid(&xy, vcx)
        && (words.len() < 3
            || Len::parse(&words[2], vcx).is_some_and(|l| {
                !matches!(
                    l,
                    Len::Pct(_) | Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
                )
            }))
}
