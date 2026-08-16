//! `border-image`: картинка вместо рамки (css-backgrounds-3 §6).
//!
//! Почему отдельным слоем, а не стилем коробки. Рамка-картинка — это девять
//! кусков одного образа: четыре угла ставятся по углам как есть, четыре края
//! растягиваются или мостятся вдоль сторон, середина рисуется только по
//! просьбе (`fill`). Ни рамка, ни фон в раскладке так не умеют, поэтому куски
//! рисует канвас, знающий свои границы во время отрисовки.
//!
//! Кусок образа рисуется через МАСКУ: отрисовка картинки в GPUI берёт образ
//! целиком, вырезать из него прямоугольник нечем. Поэтому образ рисуется
//! увеличенным и сдвинутым так, чтобы нужный кусок лёг ровно в своё место, а
//! всё остальное срезала маска куска.

use crate::computed::{Computed, Tiling};
use crate::value::Len;
use gpui::{AnyElement, Bounds, IntoElement, Pixels, Styled, px};

/// Слой рамки-картинки поверх коробки.
pub fn layer(c: &Computed) -> Option<AnyElement> {
    let image = c.border_image.clone().filter(|bi| !bi.src.is_empty())?;
    let family = c.font_family.clone().unwrap_or_default();
    let size = match c.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let border = c.borders();
    let px_of = |l: Option<Len>| crate::metrics::spacing_px(l, &family, size);
    // Толщина самой рамки нужна дважды: как умолчание ширины рамки-картинки и
    // как основа для записи её числом (`border-image-width: 2` — это два
    // значения `border-width`).
    let base = [
        px_of(border.top),
        px_of(border.right),
        px_of(border.bottom),
        px_of(border.left),
    ];
    let outset = image.outset;
    Some(
        gpui::canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let Some(found) = crate::background::source(&image.src) else {
                    return;
                };
                let intrinsic = found.intrinsic();
                // Своей величины у рисунка может не быть вовсе — тогда его
                // область просмотра равна коробке, как и у фона.
                let (iw, ih) = (
                    intrinsic.w.unwrap_or(f32::from(bounds.size.width)).max(1.0),
                    intrinsic
                        .h
                        .unwrap_or(f32::from(bounds.size.height))
                        .max(1.0),
                );
                let Some(raster) = found.raster((iw, ih)) else {
                    return;
                };
                // Слой лежит внутри коробки и меряется её ВНУТРЕННИМ краем, а
                // рамка рисуется от ВНЕШНЕГО: раздуваем на толщину рамки и на
                // `outset` сверху. Без этого вся девятка уезжала внутрь на
                // толщину рамки и накрывала содержимое.
                let area = Bounds {
                    origin: gpui::point(
                        bounds.origin.x - px(base[3] + outset[3]),
                        bounds.origin.y - px(base[0] + outset[0]),
                    ),
                    size: gpui::size(
                        bounds.size.width + px(base[1] + base[3] + outset[1] + outset[3]),
                        bounds.size.height + px(base[0] + base[2] + outset[0] + outset[2]),
                    ),
                };
                let (aw, ah) = (f32::from(area.size.width), f32::from(area.size.height));
                // Срезы образа в его же точках.
                let cut = [
                    image.slice[0].px(ih),
                    image.slice[1].px(iw),
                    image.slice[2].px(ih),
                    image.slice[3].px(iw),
                ];
                // Ширины кусков рамки на экране.
                let w = [
                    image.width[0].px(base[0], ah),
                    image.width[1].px(base[1], aw),
                    image.width[2].px(base[2], ah),
                    image.width[3].px(base[3], aw),
                ];
                // Полосы вдоль осей: угол — середина — угол.
                let cols = [w[3], (aw - w[3] - w[1]).max(0.0), w[1]];
                let rows = [w[0], (ah - w[0] - w[2]).max(0.0), w[2]];
                let src_cols = [cut[3], (iw - cut[3] - cut[1]).max(0.0), cut[1]];
                let src_rows = [cut[0], (ih - cut[0] - cut[2]).max(0.0), cut[2]];
                let x0 = f32::from(area.origin.x);
                let y0 = f32::from(area.origin.y);
                for row in 0..3usize {
                    for col in 0..3usize {
                        // Середина рисуется только по просьбе.
                        if row == 1 && col == 1 && !image.fill {
                            continue;
                        }
                        let (sw, sh) = (src_cols[col], src_rows[row]);
                        let (dw, dh) = (cols[col], rows[row]);
                        if sw <= 0.0 || sh <= 0.0 || dw <= 0.0 || dh <= 0.0 {
                            continue;
                        }
                        let sx = src_cols[..col].iter().sum::<f32>();
                        let sy = src_rows[..row].iter().sum::<f32>();
                        let dx = x0 + cols[..col].iter().sum::<f32>();
                        let dy = y0 + rows[..row].iter().sum::<f32>();
                        // Мостится ТОЛЬКО вдоль полосы: средняя колонка — по
                        // горизонтали, средний ряд — по вертикали, углы не
                        // мостятся вовсе и всегда растягиваются.
                        // Длина копии вдоль полосы масштабируется ВМЕСТЕ с
                        // поперечным растяжением (css-backgrounds-3 §6.2):
                        // кусок сохраняет свои пропорции. Верхний край при
                        // рамке вдвое толще среза мостится копиями вдвое
                        // длиннее исходных — не исходной длиной.
                        let unit = if row == 1 && col == 1 {
                            // Середина: ширина копии масштабируется как у
                            // ВЕРХНЕГО края, высота — как у ЛЕВОГО (§6.2).
                            let kx = rows[0] / src_rows[0].max(0.0001);
                            let ky = cols[0] / src_cols[0].max(0.0001);
                            (sw * kx.max(0.0001), sh * ky.max(0.0001))
                        } else if row == 1 {
                            (dw.max(1.0), sh * dw / sw.max(0.0001))
                        } else if col == 1 {
                            (sw * dh / sh.max(0.0001), dh.max(1.0))
                        } else {
                            (sw, sh)
                        };
                        let cells = pieces(
                            image.repeat,
                            (dx, dy, dw, dh),
                            unit,
                            (col == 1, row == 1),
                        );
                        if std::env::var("HTML_BI").is_ok() {
                            eprintln!(
                                "BI r{row} c{col} src=({sx},{sy},{sw},{sh}) dest=({dx:.0},{dy:.0},{dw:.0},{dh:.0}) cells={}",
                                cells.len()
                            );
                        }
                        for (cell, (fx, fy)) in cells {
                            // Источник обрезанной копии — та же доля куска.
                            let part = (
                                sx + fx.0 * sw,
                                sy + fy.0 * sh,
                                (fx.1 - fx.0) * sw,
                                (fy.1 - fy.0) * sh,
                            );
                            paint_slice(window, &raster, (iw, ih), part, cell);
                        }
                    }
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}

/// На какие прямоугольники распадается один кусок рамки.
///
/// Растянутый кусок — это один прямоугольник во всю полосу. Мостящийся (`repeat`,
/// `round`, `space`) режется на копии своего размера: вдоль полосы их столько,
/// сколько влезает, поперёк кусок всё равно растягивается.
type Cell = ((f32, f32, f32, f32), ((f32, f32), (f32, f32)));

fn pieces(
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
            // Копия не выходит за свою полосу: девятка клипается по областям
            // (css-backgrounds-3 §6.2), а `repeat` кладёт копии от середины и
            // крайние выступают. Видимой части копии отвечает та же ДОЛЯ
            // исходного куска — обрезанная плитка показывает свой край, а не
            // сжатый целый кусок.
            let (x0, x1) = (ox.max(0.0), (ox + w).min(dw));
            let (y0, y1) = (oy.max(0.0), (oy + h).min(dh));
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            let fx = ((x0 - ox) / w, (x1 - ox) / w);
            let fy = ((y0 - oy) / h, (y1 - oy) / h);
            out.push(((dx + x0, dy + y0, x1 - x0, y1 - y0), (fx, fy)));
        }
    }
    out
}

/// Копии вдоль ОДНОЙ оси: смещение и длина каждой.
fn along(mode: Tiling, span: f32, piece: f32) -> Vec<(f32, f32)> {
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
            let count = (span / piece).floor().min(MAX);
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
            let count = (span / piece).ceil().max(1.0).min(MAX);
            let first = (span - count * piece) / 2.0;
            (0..count as u32)
                .map(|i| (first + i as f32 * piece, piece))
                .collect()
        }
    }
}

/// Кэш вырезанных кусков: (образ, прямоугольник растра) → свой образ.
///
/// Кусок обязан быть ОТДЕЛЬНЫМ образом, а не регионом атласа: регион в один
/// знак, растянутый на всю сторону рамки, размывается фильтрацией с соседями
/// по атласу — зелёная кромка тонула в красной середине (`border-image-002`).
/// Вырезка на каждый кадр непозволительна, поэтому куски запоминаются.
type SliceKey = (usize, u32, u32, u32, u32, u32, u32);
type SliceCache = std::sync::Mutex<std::collections::HashMap<SliceKey, std::sync::Arc<gpui::RenderImage>>>;
static SLICES: std::sync::OnceLock<SliceCache> = std::sync::OnceLock::new();
const SLICE_CAP: usize = 256;

/// Нарисовать ОДИН кусок образа в своё место.
fn paint_slice(
    window: &mut gpui::Window,
    raster: &std::sync::Arc<gpui::RenderImage>,
    image: (f32, f32),
    src: (f32, f32, f32, f32),
    dest: (f32, f32, f32, f32),
) {
    let (sx, sy, sw, sh) = src;
    let (dx, dy, dw, dh) = dest;
    // Источник задан в точках CSS-картинки, а вырезка — в точках РАСТРА:
    // у рисунка, растрированного плотнее, они различаются.
    let s = raster.size(0);
    let (kx, ky) = (
        s.width.0 as f32 / image.0.max(1.0),
        s.height.0 as f32 / image.1.max(1.0),
    );
    // В ключе и целевой размер: тот же кусок в другой рамке растягивается
    // иначе. Размер целевого растра — в физических точках устройства, чтобы
    // на дробном масштабе кусок не мылился повторным растяжением.
    let scale = window.scale_factor();
    let (out_w, out_h) = (
        ((dw * scale).round() as u32).max(1),
        ((dh * scale).round() as u32).max(1),
    );
    let key: SliceKey = (
        std::sync::Arc::as_ptr(raster) as usize,
        (sx * kx).round() as u32,
        (sy * ky).round() as u32,
        (sw * kx).round().max(1.0) as u32,
        (sh * ky).round().max(1.0) as u32,
        out_w,
        out_h,
    );
    let cache = SLICES.get_or_init(Default::default);
    let piece = {
        let hit = cache.lock().ok().and_then(|m| m.get(&key).cloned());
        match hit {
            Some(found) => found,
            None => {
                let Some(cut) = gpui::crop_image(raster, key.1, key.2, key.3, key.4, out_w, out_h) else {
                    return;
                };
                if let Ok(mut m) = cache.lock() {
                    if m.len() >= SLICE_CAP {
                        m.clear();
                    }
                    m.insert(key, cut.clone());
                }
                cut
            }
        }
    };
    let cell = Bounds {
        origin: gpui::point(px(dx), px(dy)),
        size: gpui::size(px(dw), px(dh)),
    };
    let _ = window.paint_image(cell, gpui::Corners::default(), piece, 0, false);
}

/// Ширина куска рамки: число — во столько раз толще самой рамки, длина — как
/// есть, доля — от стороны коробки (css-backgrounds-3 §6.5).
impl crate::computed::BorderImageWidth {
    pub fn px(self, border: f32, side: f32) -> f32 {
        match self {
            crate::computed::BorderImageWidth::Times(k) => k * border,
            crate::computed::BorderImageWidth::Px(v) => v,
            crate::computed::BorderImageWidth::Pct(k) => k * side,
            // `auto` — своя величина куска образа; её мы приравниваем к рамке.
            crate::computed::BorderImageWidth::Auto => border,
        }
    }
}

/// Срез образа: число — в его собственных точках, доля — от его стороны.
impl crate::computed::BorderImageSlice {
    pub fn px(self, side: f32) -> f32 {
        match self {
            crate::computed::BorderImageSlice::Px(v) => v,
            crate::computed::BorderImageSlice::Pct(k) => k * side,
        }
    }
}

#[cfg(test)]
mod tests {
    /// Вырезка с растяжением: угловой знак образа приходит чистым, без
    /// подмешивания соседей.
    #[test]
    fn crop_keeps_the_corner_colour() {
        use image::Frame;
        // 2×2: (0,0) зелёный, остальные красные. Байты в порядке BGRA.
        let g = [0u8, 128, 0, 255];
        let r = [0u8, 0, 255, 255];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&g);
        bytes.extend_from_slice(&r);
        bytes.extend_from_slice(&r);
        bytes.extend_from_slice(&r);
        let buffer = image::RgbaImage::from_raw(2, 2, bytes).unwrap();
        let image = gpui::RenderImage::new(smallvec::SmallVec::<[Frame; 1]>::from_elem(
            Frame::new(buffer),
            1,
        ));
        let cut = gpui::crop_image(&image, 0, 0, 1, 1, 4, 4).expect("вырезка");
        let out = cut.as_bytes(0).unwrap();
        assert_eq!(out.len(), 4 * 4 * 4);
        for px in out.chunks_exact(4) {
            assert_eq!(px, &g, "весь кусок — цвет углового знака");
        }
    }
}
