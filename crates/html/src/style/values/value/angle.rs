//! Resolve CSS angle-percentage calculations in turns before stop fixup.

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

#[cfg(test)]
mod tests {
    use super::fraction;

    #[test]
    fn angles_and_percentages_share_a_turn_basis() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        assert_eq!(fraction("calc(90deg + 50%)", vcx), Some(0.75));
        assert_eq!(fraction("calc(2 * (100grad - 10%))", vcx), Some(0.3));
        assert_eq!(fraction("calc(1px + 90deg)", vcx), None);
    }
}
