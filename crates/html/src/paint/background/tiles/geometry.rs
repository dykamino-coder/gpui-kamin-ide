//! Размер и начало плитки background image.

use crate::paint::background::*;
use crate::style::computed::{BgPos, BgSize, Tiling};
use crate::style::values::value::Len;

/// Размер одной плитки в точках по правилам `background-size`.
pub(crate) fn tile_size(i: Intrinsic, box_size: (f32, f32), size: BgSize) -> (f32, f32) {
    let (bw, bh) = box_size;
    // Соотношение для растяжений: своё, иначе — из умолчального размера.
    let auto = default_size(i, box_size);
    let ratio = i
        .ratio
        .unwrap_or_else(|| if auto.1 > 0.0 { auto.0 / auto.1 } else { 1.0 });
    match size {
        BgSize::Auto => auto,
        // Без своего соотношения картинка растягивается на место под фон
        // ЦЕЛИКОМ: сохранять нечего (css-images-3 §5.3).
        BgSize::Cover | BgSize::Contain if i.ratio.is_none() => box_size,
        BgSize::Cover | BgSize::Contain => {
            let (iw, ih) = (ratio.max(0.0001), 1.0);
            let sx = bw / iw;
            let sy = bh / ih;
            // `cover` закрывает коробку целиком, `contain` вписывается в неё.
            let k = if matches!(size, BgSize::Cover) {
                sx.max(sy)
            } else {
                sx.min(sy)
            };
            (iw * k, ih * k)
        }
        // Заданная одна сторона тянет вторую по соотношению — как в CSS.
        BgSize::Fixed(w, h) => match (len_px(w, bw), len_px(h, bh)) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) if i.ratio.is_some() || i.w.is_some() => (w, w / ratio),
            (Some(w), None) => (w, auto.1),
            (None, Some(h)) if i.ratio.is_some() || i.h.is_some() => (h * ratio, h),
            (None, Some(h)) => (auto.0, h),
            (None, None) => auto,
        },
    }
}

pub(crate) fn len_px(l: Option<Len>, base: f32) -> Option<f32> {
    match l? {
        Len::Px(v) => Some(v),
        Len::Pct(v) => Some(base * v),
        Len::Calc(i) => {
            let s = crate::style::values::value::calc_get(i);
            Some(s.px + base * s.pct)
        }
        // Шрифтовые единицы — от запасного кегля, единой точкой.
        l @ (Len::Em(_)
        | Len::EmPx(..)
        | Len::Ch(_)
        | Len::Ic(_)
        | Len::Ex(_)
        | Len::Lh(_)
        | Len::LhPx(..)) => crate::text::metrics::fallback_len_px(l, "", 16.0),
        Len::Vw(_) | Len::Vh(_) => None,
        Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent | Len::Anchor(_) => None,
    }
}

/// Смещение первой плитки: проценты считаются от свободного места, как в CSS.
pub(super) fn origin(pos: BgPos, box_size: (f32, f32), tile: (f32, f32)) -> (f32, f32) {
    let axis = |l: Option<Len>, box_len: f32, tile_len: f32| -> f32 {
        match l {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(v)) => (box_len - tile_len) * v,
            // `calc(50px + 50%)`: доля — от свободного места, как у чистой
            // доли (css-backgrounds-3 §3.6), точки — как есть. Смесь с
            // третьей природой парой не отдаётся и, как прежде, идёт нулём.
            Some(Len::Calc(i)) => crate::style::values::value::calc_get(i)
                .pct_px()
                .map_or(0.0, |(pct, px)| (box_len - tile_len) * pct + px),
            _ => 0.0,
        }
    };
    (
        axis(pos.x, box_size.0, tile.0),
        axis(pos.y, box_size.1, tile.1),
    )
}

/// Размер плитки после подгонки под целое их число (`background-repeat: round`).
///
/// css-backgrounds-3 §3.4: плитка растягивается или сжимается так, чтобы вдоль
/// оси уложилось целое их число без зазоров. Одна плитка — минимум: меньше
/// целой копии не бывает.
pub(crate) fn rounded(mode: Tiling, tile: f32, box_len: f32) -> f32 {
    if mode != Tiling::Round || tile <= 0.0 || box_len <= 0.0 {
        return tile;
    }
    let count = (box_len / tile).round().max(1.0);
    box_len / count
}
