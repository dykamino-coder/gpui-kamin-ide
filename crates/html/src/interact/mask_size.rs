//! Resolve mask image sizes with the CSS background image sizing algorithm.
use crate::{
    background::{Intrinsic, tile_size},
    computed::BgSize,
    value::Len,
};

pub(super) fn tile(
    intrinsic: Intrinsic,
    scale: f32,
    area: (f32, f32),
    size: Option<(Len, Len)>,
    fit: u8,
) -> (f32, f32) {
    let intrinsic = Intrinsic {
        w: intrinsic.w.map(|w| w * scale),
        h: intrinsic.h.map(|h| h * scale),
        ..intrinsic
    };
    let size = match fit {
        1 => BgSize::Contain,
        2 => BgSize::Cover,
        _ => size.map_or(BgSize::Auto, |(w, h)| BgSize::Fixed(Some(w), Some(h))),
    };
    // CSS Masking §7.8 defines mask-size through CSS Backgrounds §3.9.
    tile_size(intrinsic, area, size)
}
