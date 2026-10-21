//! Разбор `<angle>` и `<angle-percentage>` в долях оборота: `deg`/`grad`/`rad`/`turn`/`%` и
//! их `calc()`/`min()`/`max()` (CSS Values 4: 100% — один оборот) через общий разборщик
//! математики с единой размерностью. Конические градиенты берут долю, трансформации и
//! motion — градусы чистого `<angle>`.

use crate::document::state::ValueCx;
use crate::style::values::value::Len;

pub(crate) fn fraction(token: &str, vcx: ValueCx<'_>) -> Option<f32> {
    let token = token.trim();
    if token.starts_with("calc(") || token.starts_with("min(") || token.starts_with("max(") {
        // CSS Values 4 §angle-percentage: 100% is one turn. Map both
        // dimensions to a common length so the existing math parser retains
        // multiplication, nested functions, and their validation.
        let mut expression = String::new();
        let mut at = 0;
        let bytes = token.as_bytes();
        while at < bytes.len() {
            let start = at;
            if bytes[at].is_ascii_digit() || bytes[at] == b'.' {
                while at < bytes.len() && (bytes[at].is_ascii_digit() || bytes[at] == b'.') {
                    at += 1;
                }
                let end = at;
                while at < bytes.len() && (bytes[at].is_ascii_alphabetic() || bytes[at] == b'%') {
                    at += 1;
                }
                if at > end {
                    expression.push_str(&format!("{}px", fraction(&token[start..at], vcx)?));
                } else {
                    expression.push_str(&token[start..at]);
                }
            } else {
                expression.push(bytes[at] as char);
                at += 1;
            }
        }
        return match Len::parse(&expression, vcx)? {
            Len::Px(v) => Some(v),
            _ => None,
        };
    }
    for (unit, divisor) in [
        ("%", 100.0),
        ("deg", 360.0),
        ("grad", 400.0),
        ("rad", std::f32::consts::TAU),
        ("turn", 1.0),
    ] {
        if let Some(number) = token.strip_suffix(unit) {
            return number.parse::<f32>().ok().map(|n| n / divisor);
        }
    }
    (token == "0").then_some(0.0)
}

/// Чистый `<angle>` в градусах (CSS Values 4 §angles): проценты не допускаются,
/// безразмерный ноль — только где грамматика добавляет `<zero>` (функции
/// css-transforms-1 §transform-functions), но не в `rotate`/`offset-rotate`.
pub(crate) fn degrees(token: &str, vcx: ValueCx<'_>, unitless_zero: bool) -> Option<f32> {
    let token = token.trim().to_ascii_lowercase();
    if token.contains('%') {
        return None;
    }
    if token.parse::<f32>().is_ok() {
        return (unitless_zero && token.parse::<f32>() == Ok(0.0)).then_some(0.0);
    }
    let math = token.starts_with("calc(") || token.starts_with("min(") || token.starts_with("max(");
    if !math
        && !["deg", "grad", "rad", "turn"].iter().any(|unit| {
            token
                .strip_suffix(unit)
                .is_some_and(|n| n.parse::<f32>().is_ok())
        })
    {
        return None;
    }
    fraction(&token, vcx)
        .map(|turns| turns * 360.0)
        .filter(|d| d.is_finite())
}

#[cfg(test)]
mod tests {
    use super::{degrees, fraction};

    #[test]
    fn plain_angles_cover_every_unit_and_range() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        assert_eq!(degrees("180deg", vcx, false), Some(180.0));
        assert_eq!(degrees("400grad", vcx, false), Some(360.0));
        assert_eq!(degrees("-0.5turn", vcx, false), Some(-180.0));
        assert_eq!(degrees("135DEG", vcx, false), Some(135.0));
        assert!(degrees("3.14159rad", vcx, false).is_some_and(|d| (d - 180.0).abs() < 0.01));
        assert_eq!(degrees("calc(90deg * 2)", vcx, false), Some(180.0));
        assert_eq!(degrees("0", vcx, true), Some(0.0));
        assert_eq!(degrees("0", vcx, false), None);
        assert_eq!(degrees("45", vcx, true), None);
        assert_eq!(degrees("50%", vcx, true), None);
        assert_eq!(degrees("10px", vcx, true), None);
    }

    #[test]
    fn angles_and_percentages_share_a_turn_basis() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        assert_eq!(fraction("calc(90deg + 50%)", vcx), Some(0.75));
        assert_eq!(fraction("calc(2 * (100grad - 10%))", vcx), Some(0.3));
        assert_eq!(fraction("calc(1px + 90deg)", vcx), None);
    }
}
