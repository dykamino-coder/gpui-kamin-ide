//! Resolve mask image sizes with the CSS background image sizing algorithm.
use crate::paint::background::{Intrinsic, tile_size};
use crate::style::computed::BgSize;
use crate::style::values::value::Len;
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

/// `round` (css-backgrounds-3 §3.4, via css-masking-1 §7.6): the tile is
/// rescaled so a whole number of copies fills the positioning area,
/// `max(1, round(area / tile))`. When only one axis rounds and the other
/// `mask-size` component is `auto`, that axis keeps the aspect ratio.
pub(super) fn round_tile(
    (tw, th): (f32, f32),
    (bw, bh): (f32, f32),
    (rx, ry): (bool, bool),
    size: Option<(Len, Len)>,
) -> (f32, f32) {
    let fit = |t: f32, area: f32| {
        if t <= 0.0 || area <= 0.0 {
            return t;
        }
        area / (area / t).round().max(1.0)
    };
    let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
    let (nw, nh) = (if rx { fit(tw, bw) } else { tw }, if ry { fit(th, bh) } else { th });
    match (rx, ry) {
        (true, false) if auto(size.map(|s| s.1)) && tw > 0.0 => (nw, th * nw / tw),
        (false, true) if auto(size.map(|s| s.0)) && th > 0.0 => (tw * nh / th, nh),
        _ => (nw, nh),
    }
}

/// `space` along one axis: `(start, gap, single)`. With at least two whole
/// tiles the first touches the start edge and the rest share the leftover
/// equally; otherwise one tile stays at its `mask-position` offset.
pub(super) fn space_axis(space: bool, pos: f32, tile: f32, area: f32) -> (f32, f32, bool) {
    if !space || tile <= 0.0 {
        return (pos, 0.0, false);
    }
    let n = (area / tile).floor();
    if n < 2.0 {
        return (pos, 0.0, true);
    }
    (0.0, (area - n * tile) / (n - 1.0), false)
}
