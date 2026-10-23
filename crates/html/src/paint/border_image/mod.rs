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

use crate::document::state::ValueCx;
mod gradient;
mod grid;
mod tiling;
use tiling::pieces;

use crate::style::computed::Computed;
use crate::style::values::value::Len;
mod sampling;
use crate::paint::border_image::sampling::{destination_bounds, paint_slice};
use gpui::{AnyElement, Bounds, IntoElement, Pixels, Styled, px};

/// Слой рамки-картинки поверх коробки.
pub fn layer(c: &Computed, vcx: ValueCx<'_>) -> Option<AnyElement> {
    let values = vcx.keep();
    let image = c.border_image.clone().filter(|bi| bi.src.is_some())?;
    let family = c.font_family.clone().unwrap_or_default();
    let size = match c.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let border = c.borders();
    let px_of = |l: Option<Len>| crate::text::metrics::spacing_px(l, &family, size, vcx);
    // Толщина самой рамки нужна дважды: как умолчание ширины рамки-картинки и
    // как основа для записи её числом (`border-image-width: 2` — это два
    // значения `border-width`).
    let base = [
        px_of(border.top),
        px_of(border.right),
        px_of(border.bottom),
        px_of(border.left),
    ];
    let outset = std::array::from_fn::<_, 4, _>(|i| image.outset[i].px(base[i], 0.0));
    let src = crate::resources::css_images::computed_source(
        image.src.as_ref()?,
        c.image_orient_none != Some(true),
        vcx,
    )
    .map(crate::paint::background::Source::from_cpu)?;
    let pixelated = c.image_pixelated == Some(true);
    Some(
        gpui::canvas_with_unrounded_bounds(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let vcx = values.vcx();
                let found = &src;
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
                let grid = grid::Grid::new(&image, &found, area, base);
                let (iw, ih) = grid.image;
                let Some(raster) = found.raster((iw, ih), vcx) else {
                    return;
                };
                let (cols, rows) = (grid.cols, grid.rows);
                let (src_cols, src_rows) = (grid.src_cols, grid.src_rows);
                let x0 = f32::from(grid.area.origin.x);
                let y0 = f32::from(grid.area.origin.y);
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
                        let sx = grid.source_x[col];
                        let sy = grid.source_y[row];
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
                        for (cell, visible) in cells {
                            let clip = destination_bounds(window, visible);
                            window.with_content_mask(
                                Some(gpui::ContentMask { bounds: clip }),
                                |window| {
                                    if gradient::paint(
                                        window,
                                        &found,
                                        (iw, ih),
                                        (sx, sy, sw, sh),
                                        cell,
                                        vcx,
                                    ) {
                                        return;
                                    }
                                    paint_slice(
                                        window,
                                        &raster,
                                        (iw, ih),
                                        matches!(
                                            found,
                                            crate::paint::background::Source::Raster(_)
                                        ),
                                        (sx, sy, sw, sh),
                                        cell,
                                        pixelated,
                                    );
                                },
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
mod tests {
    //! Проверки контракта родительского модуля; вынесены для ограничения размера файлов.

    #[test]
    fn repeat_centers_one_full_tile_and_clips_its_neighbours() {
        use crate::style::computed::Tiling;
        let cells = super::tiling::pieces(
            (Tiling::Repeat, Tiling::None),
            (0.0, 0.0, 80.0, 10.0),
            (40.0, 10.0),
            (true, false),
        );
        assert_eq!(cells.len(), 3);
        assert_eq!(cells[0], ((-20.0, 0.0, 40.0, 10.0), (0.0, 0.0, 20.0, 10.0)));
        assert_eq!(cells[1], ((20.0, 0.0, 40.0, 10.0), (20.0, 0.0, 40.0, 10.0)));
        assert_eq!(cells[2], ((60.0, 0.0, 40.0, 10.0), (60.0, 0.0, 20.0, 10.0)));
    }

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
