//! Repeat offsets retain the source phase across fractional clipping bounds.
//!
//! CSS Backgrounds 3 §3.4 preserves the positioning area's tile grid. Keep
//! clipping offsets in f64 until they cancel when forming absolute coordinates:
//! an f32 round trip through the device-snapped clip perturbs exact texel ties.
//! Blink background_image_geometry.cc:179-191 likewise keeps repeat phase
//! unsnapped, independently of the paint rectangle.

use super::{MAX_TILES, Tiling};

pub(super) fn axis(mode: Tiling, from: f32, tile: f32, shift: f64, own: f32, all: f32) -> Vec<f64> {
    let (from, tile, own, all) = (
        f64::from(from),
        f64::from(tile),
        f64::from(own),
        f64::from(all),
    );
    if mode == Tiling::Space {
        let base = tiling(mode, from, tile, own);
        // За областью позиционирования плитки продолжаются с тем же
        // шагом по всей области покраски (css-backgrounds-3 §3.4:
        // «…continue to be repeated at the same spacing»), иначе под
        // рамкой пусто (`background-repeat-space-8`).
        let step = match base.as_slice() {
            [a, b, ..] => b - a,
            _ => tile,
        };
        let mut out = Vec::new();
        if step > 0.0 {
            let (first, last) = (base[0], base[base.len() - 1]);
            let mut v = first - step;
            let mut n = 0;
            while v + tile > -shift && n < MAX_TILES as usize {
                out.push(v);
                v -= step;
                n += 1;
            }
            out.reverse();
            out.extend(base.iter().copied());
            let mut v = last + step;
            let mut n = 0;
            while v < all - shift && n < MAX_TILES as usize {
                out.push(v);
                v += step;
                n += 1;
            }
        } else {
            out = base;
        }
        out.into_iter().map(|v| v + shift).collect()
    } else {
        tiling(mode, from + shift, tile, all)
    }
}

/// One coordinate per copy: space distribution cannot be expressed as a tile
/// count and one offset, because the intervening gaps also depend on the box.
pub(super) fn tiling(mode: Tiling, start: f64, tile: f64, box_len: f64) -> Vec<f64> {
    let max = f64::from(MAX_TILES);
    match mode {
        Tiling::None => vec![start],
        // Зазоры раздаются между ЦЕЛЫМИ плитками, крайние прижаты к краям, а
        // `background-position` вдоль этой оси не действует. Если целиком
        // влезает меньше двух — плитка одна и смещение своё (§3.4).
        Tiling::Space => {
            let fit = (box_len / tile).floor();
            if fit < 2.0 {
                return vec![start];
            }
            let count = fit.min(max);
            let gap = (box_len - count * tile) / (count - 1.0);
            (0..count as u32)
                .map(|i| f64::from(i) * (tile + gap))
                .collect()
        }
        // `round` уже подогнал размер плитки — дальше это обычная кладка.
        Tiling::Repeat | Tiling::Round => {
            // Начало сдвигается назад на целое число плиток, иначе смещение
            // съедало бы первый ряд.
            let back = (start / tile).ceil();
            let first = start - back * tile;
            let count = ((box_len - first) / tile).ceil().max(1.0).min(max);
            (0..count as u32)
                .map(|i| first + f64::from(i) * tile)
                .collect()
        }
    }
}
