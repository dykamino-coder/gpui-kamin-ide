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

mod tiling;
use tiling::pieces;

use crate::style::computed::Computed;
use crate::style::values::value::Len;
mod sampling;
use crate::paint::border_image::sampling::paint_slice;
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
    let px_of = |l: Option<Len>| crate::text::metrics::spacing_px(l, &family, size);
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
    // `image-orientation` действует и на рамку-картинку (css-images-3 §5.4,
    // «Applies to: all elements»). Ключ строится ДО замыкания: в него уезжает
    // готовая строка, а не стиль.
    let src = crate::paint::background::key_exif(&image.src, c);
    let pixelated = c.image_pixelated == Some(true);
    Some(
        gpui::canvas_with_unrounded_bounds(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let Some(found) = crate::paint::background::source(&src) else {
                    return;
                };
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
                let intrinsic = found.intrinsic();
                // Своей величины у рисунка может не быть вовсе — тогда размер
                // по умолчанию — ОБЛАСТЬ РАМКИ (css-backgrounds-3 §6.2), а не
                // padding-box: у коробки 0×0 растр был 1×1
                // (`border-image-image-type-004/005`).
                let (iw, ih) = (
                    intrinsic.w.unwrap_or(f32::from(area.size.width)).max(1.0),
                    intrinsic.h.unwrap_or(f32::from(area.size.height)).max(1.0),
                );
                let Some(raster) = found.raster((iw, ih)) else {
                    return;
                };
                // Слой лежит внутри коробки и меряется её ВНУТРЕННИМ краем, а
                // рамка рисуется от ВНЕШНЕГО: раздуваем на толщину рамки и на
                // `outset` сверху. Без этого вся девятка уезжала внутрь на
                // толщину рамки и накрывала содержимое.
                let (aw, ah) = (f32::from(area.size.width), f32::from(area.size.height));
                // Срезы образа в его же точках.
                let cut = [
                    image.slice[0].px(ih),
                    image.slice[1].px(iw),
                    image.slice[2].px(ih),
                    image.slice[3].px(iw),
                ];
                // Ширины кусков рамки на экране. `auto` — своя величина куска
                // образа (css-backgrounds-3 §6.5: «the intrinsic width or
                // height … of the corresponding image slice»), а у образа без
                // своей величины — толщина рамки (`BorderImageWidth::px`).
                let own_size = intrinsic.w.is_some() && intrinsic.h.is_some();
                let one = |i: usize, border: f32, side: f32| match image.width[i] {
                    crate::style::computed::BorderImageWidth::Auto if own_size => cut[i],
                    other => other.px(border, side),
                };
                let mut w = [
                    one(0, base[0], ah),
                    one(1, base[1], aw),
                    one(2, base[2], ah),
                    one(3, base[3], aw),
                ];
                // Встречные ширины не перекрываются (§6.5): «the used values of
                // all border-image-width values are proportionally reduced
                // until they no longer overlap».
                let f = (aw / (w[1] + w[3]).max(0.0001)).min(ah / (w[0] + w[2]).max(0.0001));
                if f < 1.0 {
                    for v in w.iter_mut() {
                        *v *= f;
                    }
                }
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
                            // Вырожденный множитель (край нулевой толщины —
                            // `border-style: none`) считается единицей: копия
                            // идёт своими точками и центрируется, в окно
                            // попадает середина образа (`slice-fill-003`).
                            let kx = if rows[0] > 0.0 && src_rows[0] > 0.0 {
                                rows[0] / src_rows[0]
                            } else {
                                1.0
                            };
                            let ky = if cols[0] > 0.0 && src_cols[0] > 0.0 {
                                cols[0] / src_cols[0]
                            } else {
                                1.0
                            };
                            (sw * kx, sh * ky)
                        } else if row == 1 {
                            (dw.max(1.0), sh * dw / sw.max(0.0001))
                        } else if col == 1 {
                            (sw * dh / sh.max(0.0001), dh.max(1.0))
                        } else {
                            (sw, sh)
                        };
                        let cells =
                            pieces(image.repeat, (dx, dy, dw, dh), unit, (col == 1, row == 1));
                        for (cell, (fx, fy)) in cells {
                            // Источник обрезанной копии — та же доля куска.
                            let part = (
                                sx + fx.0 * sw,
                                sy + fy.0 * sh,
                                (fx.1 - fx.0) * sw,
                                (fy.1 - fy.0) * sh,
                            );
                            paint_slice(
                                window,
                                &raster,
                                (iw, ih),
                                matches!(found, crate::paint::background::Source::Raster(_)),
                                part,
                                cell,
                                pixelated,
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

/// Ширина куска рамки: число — во столько раз толще самой рамки, длина — как
/// есть, доля — от стороны коробки (css-backgrounds-3 §6.5).
impl crate::style::computed::BorderImageWidth {
    pub fn px(self, border: f32, side: f32) -> f32 {
        match self {
            crate::style::computed::BorderImageWidth::Times(k) => k * border,
            crate::style::computed::BorderImageWidth::Px(v) => v,
            crate::style::computed::BorderImageWidth::Pct(k) => k * side,
            // `auto` — своя величина куска образа; её мы приравниваем к рамке.
            crate::style::computed::BorderImageWidth::Auto => border,
        }
    }
}

/// Срез образа: число — в его собственных точках, доля — от его стороны.
impl crate::style::computed::BorderImageSlice {
    pub fn px(self, side: f32) -> f32 {
        match self {
            crate::style::computed::BorderImageSlice::Px(v) => v,
            crate::style::computed::BorderImageSlice::Pct(k) => k * side,
        }
    }
}

#[cfg(test)]
mod tests;
