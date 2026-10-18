//! Select and inset a single circular CSS corner in its original border box.

use gpui::{Bounds, Pixels, px};

pub(crate) use crate::style::computed::single_corner::eligible;
pub(super) use crate::style::computed::single_corner::uniform;

pub(super) fn inset(r: Bounds<Pixels>, n: f32) -> Bounds<Pixels> {
    Bounds::from_corners(
        gpui::point(r.left() + px(n), r.top() + px(n)),
        gpui::point(r.right() - px(n), r.bottom() - px(n)),
    )
}

pub(super) fn virtual_circle(r: Bounds<Pixels>, index: usize, radius: f32) -> Bounds<Pixels> {
    let x = if matches!(index, 1 | 2) {
        r.right() - px(2.0 * radius)
    } else {
        r.left()
    };
    let y = if matches!(index, 2 | 3) {
        r.bottom() - px(2.0 * radius)
    } else {
        r.top()
    };
    Bounds {
        origin: gpui::point(x, y),
        size: gpui::size(px(2.0 * radius), px(2.0 * radius)),
    }
}

pub(super) fn square(r: Bounds<Pixels>, i: usize, radius: f32) -> Bounds<Pixels> {
    let x = if matches!(i, 1 | 2) {
        r.right() - px(radius)
    } else {
        r.left()
    };
    let y = if matches!(i, 2 | 3) {
        r.bottom() - px(radius)
    } else {
        r.top()
    };
    Bounds {
        origin: gpui::point(x, y),
        size: gpui::size(px(radius), px(radius)),
    }
}
