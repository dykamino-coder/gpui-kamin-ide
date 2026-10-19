//! Целое значение `z-index`.

use crate::document::state::ValueCx;

/// Целое за пределами i32 КЛАМПИТСЯ, а не падает в auto (z-index-001:
/// -2147483649 обязан остаться меньше -100).
pub(super) fn integer(v: &str, vcx: ValueCx<'_>) -> Option<i32> {
    v.parse::<i64>()
        .ok()
        .map(|n| n.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
        // `<integer>` from a math function (css-values-4 §10.9):
        // rounded to the nearest integer, halves toward +∞
        // (`calc-positive-fraction-001`: `calc(3 / 2)` → 2).
        .or_else(|| {
            (!v.trim_start()
                .starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+'))
            .then(|| crate::style::values::value::number(v, vcx))
            .flatten()
            .map(|x| {
                let r = if x.is_nan() {
                    0.0
                } else {
                    (x as f64 + 0.5).floor()
                };
                r.clamp(i32::MIN as f64, i32::MAX as f64) as i32
            })
        })
}
