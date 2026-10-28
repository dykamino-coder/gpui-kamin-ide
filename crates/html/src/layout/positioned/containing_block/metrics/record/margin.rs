//! Смещение static margin edge в исходной normal-геометрии.

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use gpui::{Pixels, Point, point, px};

pub(super) fn margin_offset(style: &Computed, rtl: bool, vcx: ValueCx<'_>) -> Point<Pixels> {
    use crate::layout::block::struts::margin_px;
    // CSS2 §10.3.7: static right задаёт правый margin edge.
    let x = if rtl {
        style.margin.right
    } else {
        style.margin.left
    };
    point(
        px(margin_px(x, style, vcx).unwrap_or(0.) * if rtl { -1. } else { 1. }),
        px(margin_px(style.margin.top, style, vcx).unwrap_or(0.)),
    )
}
