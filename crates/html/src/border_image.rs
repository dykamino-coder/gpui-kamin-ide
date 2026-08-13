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
    let image = c.border_image.clone()?;
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
                        let cells = pieces(
                            image.repeat,
                            (dx, dy, dw, dh),
                            (sw, sh),
                            (col == 1, row == 1),
                        );
                        for cell in cells {
                            paint_slice(
                                window,
                                &raster,
                                (iw, ih),
                                (sx, sy, sw, sh),
                                cell,
                            );
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
fn pieces(
    mode: (Tiling, Tiling),
    dest: (f32, f32, f32, f32),
    src: (f32, f32),
    tile: (bool, bool),
) -> Vec<(f32, f32, f32, f32)> {
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
            out.push((dx + ox, dy + oy, *w, *h));
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

/// Нарисовать ОДИН кусок образа в своё место.
///
/// Вырезать прямоугольник из образа нечем, поэтому образ рисуется целиком —
/// увеличенным во столько раз, во сколько кусок отличается от места, и
/// сдвинутым так, чтобы кусок встал ровно. Лишнее срезает маска.
fn paint_slice(
    window: &mut gpui::Window,
    raster: &std::sync::Arc<gpui::RenderImage>,
    image: (f32, f32),
    src: (f32, f32, f32, f32),
    dest: (f32, f32, f32, f32),
) {
    let (sx, sy, sw, sh) = src;
    let (dx, dy, dw, dh) = dest;
    let (kx, ky) = (dw / sw, dh / sh);
    let whole = Bounds {
        origin: gpui::point(px(dx - sx * kx), px(dy - sy * ky)),
        size: gpui::size(px(image.0 * kx), px(image.1 * ky)),
    };
    let mask = Bounds {
        origin: gpui::point(px(dx), px(dy)),
        size: gpui::size(px(dw), px(dh)),
    };
    window.with_content_mask(Some(gpui::ContentMask { bounds: mask }), |window| {
        let _ = window.paint_image(whole, gpui::Corners::default(), raster.clone(), 0, false);
    });
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
