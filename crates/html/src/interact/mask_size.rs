//! Resolve mask image sizes with the CSS background image sizing algorithm.
use crate::{
    background::{Intrinsic, tile_size},
    computed::BgSize,
    value::Len,
};
use gpui::{Bounds, Pixels, Point, point, px, size};

pub(super) fn snap_tile(at: Point<Pixels>, tile: (f32, f32), scale: f32) -> Bounds<Pixels> {
    // CSS Masking §7.7-7.8 uses background image geometry. Snap both
    // destination edges, as Blink background_image_geometry.cc:115-122 does,
    // so a fractional device size keeps the same coverage as a painted image.
    let edge = |v: Pixels| px((f32::from(v) * scale).round() / scale);
    let origin = point(edge(at.x), edge(at.y));
    Bounds {
        origin,
        size: size(
            edge(at.x + px(tile.0.max(1.0))) - origin.x,
            edge(at.y + px(tile.1.max(1.0))) - origin.y,
        ),
    }
}

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
