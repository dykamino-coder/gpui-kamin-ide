//! Геометрия плитки фоновой картинки: размер по `background-size` (CSS Backgrounds 3 §3.9,
//! `cover`/`contain` и перенос одной стороны через природное соотношение), смещение первой
//! плитки по `background-position` от свободного места (§3.6) и подгонка размера под целое
//! число копий для `background-repeat: round` (§3.4).

use crate::document::state::ValueCx;
use crate::paint::background::*;
use crate::style::computed::FontCx;
use crate::style::computed::{BgPos, BgSize, Tiling};
use crate::style::values::value::Len;

/// Размер одной плитки в точках по правилам `background-size`.
pub(crate) fn tile_size(
    i: Intrinsic,
    box_size: (f32, f32),
    size: BgSize,
    vcx: ValueCx<'_>,
) -> (f32, f32) {
    let (bw, bh) = box_size;
    // CSS Backgrounds 3 §3.9: only a natural ratio can transfer a specified
    // dimension to an auto one. The default sizing result supplies missing
    // dimensions, but does not create a natural aspect ratio (Blink
    // BackgroundImageGeometry::CalculateFillTileSize).
    let auto = default_size(i, box_size);
    let ratio = i.ratio.unwrap_or(1.0);
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
        BgSize::Fixed(w, h) => match (len_px(w, bw, vcx), len_px(h, bh, vcx)) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) if i.ratio.is_some() => (w, w / ratio),
            (Some(w), None) => (w, i.h.unwrap_or(bh)),
            (None, Some(h)) if i.ratio.is_some() => (h * ratio, h),
            (None, Some(h)) => (i.w.unwrap_or(bw), h),
            (None, None) => auto,
        },
    }
}

/// Длина плитки против базы `base` (сторона области); шрифтовые единицы
/// в записи картинки уже решены в точки (`resolve_image_lengths`), остаток —
/// запасным контекстом.
pub(crate) fn len_px(l: Option<Len>, base: f32, vcx: ValueCx<'_>) -> Option<f32> {
    l?.to_px(Some(base), FontCx::FALLBACK, vcx)
}

/// Смещение первой плитки: проценты считаются от свободного места, как в CSS.
pub(super) fn origin(
    pos: BgPos,
    box_size: (f32, f32),
    tile: (f32, f32),
    vcx: ValueCx<'_>,
) -> (f32, f32) {
    let axis = |l: Option<Len>, box_len: f32, tile_len: f32| -> f32 {
        match l {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(v)) => (box_len - tile_len) * v,
            // `calc(50px + 50%)`: доля — от свободного места, как у чистой
            // доли (css-backgrounds-3 §3.6), точки — как есть. Смесь с
            // третьей природой парой не отдаётся и, как прежде, идёт нулём.
            Some(Len::Calc(i)) => crate::style::values::value::calc_get(i, vcx)
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
