//! Capture typed comparison expressions for background-position axes and edge offsets.

use super::PositionExpression;
use crate::document::state::ValueCx;
use crate::style::computed::split_outside_parens;
use crate::style::values::value::Len;

pub(super) fn math(raw: &str, vcx: ValueCx<'_>) -> Option<PositionExpression> {
    PositionExpression::parse(raw, vcx)
}

pub(super) fn capture(raw: &str, vcx: ValueCx<'_>) -> [Option<PositionExpression>; 2] {
    let first = crate::style::css::split_args(raw)
        .into_iter()
        .next()
        .unwrap_or("");
    let words = split_outside_parens(first);
    let keyword = |s: &str| matches!(s, "left" | "right" | "top" | "bottom" | "center");
    let mut result = [None, None];
    if words.len() >= 3 {
        let mut i = 0;
        while i < words.len() {
            let side = words[i].as_str();
            if !keyword(side) {
                return [None, None];
            }
            i += 1;
            let offset = words.get(i).filter(|s| !keyword(s));
            if let Some(offset) = offset {
                if Len::parse_mixed(offset, vcx).is_none() && math(offset, vcx).is_none() {
                    return [None, None];
                }
                i += 1;
                if let Some(expression) = math(offset, vcx)
                    && side != "center"
                {
                    let axis = usize::from(matches!(side, "top" | "bottom"));
                    result[axis] = Some(if matches!(side, "right" | "bottom") {
                        expression.offset_from_end()
                    } else {
                        expression
                    });
                }
            }
        }
        return result;
    }
    let fixed = [
        words.iter().any(|w| matches!(w.as_str(), "left" | "right")),
        words.iter().any(|w| matches!(w.as_str(), "top" | "bottom")),
    ];
    let mut values = words
        .iter()
        .filter(|w| !matches!(w.as_str(), "left" | "right" | "top" | "bottom"));
    for axis in 0..2 {
        if !fixed[axis] {
            result[axis] = values.next().and_then(|v| math(v, vcx));
        }
    }
    result
}
