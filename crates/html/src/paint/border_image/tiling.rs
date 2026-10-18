//! Разбиение border-image на ячейки и повторение полос.

use crate::style::computed::Tiling;

/// На какие прямоугольники распадается один кусок рамки.
///
/// Растянутый кусок — это один прямоугольник во всю полосу. Мостящийся (`repeat`,
/// `round`, `space`) режется на копии своего размера: вдоль полосы их столько,
/// сколько влезает, поперёк кусок всё равно растягивается.
/// Full destination tile and the visible region; clipping must not resize it.
pub(super) type Cell = ((f32, f32, f32, f32), (f32, f32, f32, f32));

pub(super) fn pieces(
    mode: (Tiling, Tiling),
    dest: (f32, f32, f32, f32),
    src: (f32, f32),
    tile: (bool, bool),
) -> Vec<Cell> {
    let (dx, dy, dw, dh) = dest;
    let xs = if tile.0 {
        along(mode.0, dw, src.0)
    } else {
        vec![(0.0, dw)]
    };
    let ys = if tile.1 {
        along(mode.1, dh, src.1)
    } else {
        vec![(0.0, dh)]
    };
    let mut out = vec![];
    for (oy, h) in &ys {
        for (ox, w) in &xs {
            // CSS Backgrounds 3 §6.6 clips tiles to their nine-piece region.
            // Keep the full tile: rounding a fractional source crop before
            // scaling changes its texel mapping (Blink DrawImageTiled).
            let (x0, x1) = (ox.max(0.0), (ox + w).min(dw));
            let (y0, y1) = (oy.max(0.0), (oy + h).min(dh));
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            out.push((
                (dx + ox, dy + oy, *w, *h),
                (dx + x0, dy + y0, x1 - x0, y1 - y0),
            ));
        }
    }
    out
}

/// Копии вдоль ОДНОЙ оси: смещение и длина каждой.
// max/min also bound NaN to a finite count; f32::clamp would propagate it.
#[allow(clippy::manual_clamp)]
pub(super) fn along(mode: Tiling, span: f32, piece: f32) -> Vec<(f32, f32)> {
    // Потолок на число копий: битый срез иначе просит миллионы.
    const MAX: f32 = 512.0;
    if piece <= 0.0 || span <= 0.0 {
        return vec![(0.0, span)];
    }
    match mode {
        // `stretch` — одна копия во всю полосу.
        Tiling::None => vec![(0.0, span)],
        // `round` подгоняет саму копию под целое их число.
        Tiling::Round => {
            let count = (span / piece).round().max(1.0).min(MAX);
            let step = span / count;
            (0..count as u32).map(|i| (i as f32 * step, step)).collect()
        }
        // `space` кладёт целые копии и раздаёт остаток зазорами, в том числе
        // по краям (css-backgrounds-3 §6.2 — не так, как у фона).
        Tiling::Space => {
            // Число копий — от округлённой длины: раскладка снэпит коробку к
            // девайс-сетке (при 125% масштабе 135 становится 134.8), и целая
            // копия терялась на ровно влезающих полосах (`space-5`: две
            // вместо трёх). Сами копии и зазоры считаются от настоящей длины.
            let count = (span.round() / piece + 1e-3).floor().min(MAX);
            if count < 1.0 {
                return vec![];
            }
            let gap = (span - count * piece) / (count + 1.0);
            (0..count as u32)
                .map(|i| (gap + i as f32 * (piece + gap), piece))
                .collect()
        }
        // `repeat` мостит копиями своего размера от СЕРЕДИНЫ полосы.
        Tiling::Repeat => {
            // §6.6 step 3 centers ONE tile, independent of tile-count parity.
            // Blink ComputeTileParameters uses (destination - source) / 2.
            let center = (span - piece) / 2.0;
            let first = center - (center / piece).ceil() * piece;
            let count = ((span - first) / piece).ceil().max(1.0).min(MAX);
            (0..count as u32)
                .map(|i| (first + i as f32 * piece, piece))
                .collect()
        }
    }
}
