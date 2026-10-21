//! Координаты копий плитки вдоль оси по `background-repeat` (`repeat`, `round`, `space`,
//! `no-repeat`) с сохранением фазы источника при дробных границах обрезки.
//!
//! CSS Backgrounds 3 §3.4 сохраняет сетку плиток области позиционирования. Смещения обрезки
//! держатся в f64, пока не сократятся при образовании абсолютных координат: проход через f32 и
//! привязанную к устройству обрезку сбивает точные совпадения с границами текселей. Blink
//! `background_image_geometry.cc:179-191` так же держит фазу повтора непривязанной, независимо
//! от прямоугольника покраски. `space` при менее чем двух целых плитках откатывается к
//! `no-repeat`, а за областью позиционирования продолжает тот же шаг.

use super::MAX_TILES;
use crate::style::computed::Tiling;

pub(super) fn axis(mode: Tiling, from: f32, tile: f32, shift: f64, own: f32, all: f32) -> Vec<f64> {
    let (from, tile, own, all) = (
        f64::from(from),
        f64::from(tile),
        f64::from(own),
        f64::from(all),
    );
    if mode == Tiling::Space {
        let base = tiling(mode, from, tile, own);
        // CSS Backgrounds 3 §3.4: fewer than two whole tiles falls back to
        // no-repeat, even when the painting area extends past the origin box.
        // Blink CalculateRepeatAndPosition switches SpaceFill to NoRepeatFill.
        if base.len() < 2 {
            return base.into_iter().map(|v| v + shift).collect();
        }
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

#[cfg(test)]
mod tests {
    use super::axis;
    use crate::style::computed::Tiling;

    #[test]
    fn space_fallback_keeps_one_tile_inside_an_extended_paint_area() {
        assert_eq!(axis(Tiling::Space, 5.0, 32.0, 20.0, 48.0, 88.0), vec![25.0]);
        assert_eq!(
            axis(Tiling::Space, -10.0, 50.0, 20.0, 32.0, 72.0),
            vec![10.0]
        );
        assert_eq!(
            axis(Tiling::Space, 5.0, 32.0, 20.0, 106.0, 146.0),
            vec![-17.0, 20.0, 57.0, 94.0, 131.0]
        );
    }
}
