//! Одинаковая обёртка исходной и пересобранной печатной копии.

use gpui::{Div, Styled, div, px};

pub(super) fn wrap(
    (left, right, top): (f32, f32, f32),
    height: f32,
    first: bool,
    vertical: Option<bool>,
) -> Div {
    match vertical {
        Some(rl) => {
            // CSS Break 3 §5.4: inline-start повторяется, block-start
            // снятой корневой обёртки принадлежит только первому ребёнку.
            let d = div().flex().h(px(height)).pt(px(top));
            match (rl, first) {
                (true, true) => d.flex_row_reverse().pr(px(right)),
                (true, false) => d.flex_row_reverse(),
                (false, true) => d.flex_row().pl(px(left)),
                (false, false) => d.flex_row(),
            }
        }
        None => div()
            .pl(px(left))
            .pr(px(right))
            .pt(px(if first { top } else { 0.0 })),
    }
}
