//! Фоновая картинка: `background-image: url(...)` со всей её механикой.
//!
//! Почему отдельным проходом, а не элементом `img`. Фон в CSS — это заливка:
//! она мостится плитками, смещается, масштабируется и обрезается по коробке,
//! причём независимо от содержимого элемента. Элемент-картинка так не умеет:
//! он рисует ровно одну копию и участвует в раскладке. Поэтому фон рисуется
//! канвасом, который знает свои границы во время отрисовки, и кладёт нужное
//! число копий сам.
//!
//! Готовые CPU-источники приходят из resources; здесь только геометрия заливки
//! и загрузка точек в GPUI, без URL-кэша и декодера.

use crate::document::state::ValueCx;
mod root_area;
mod root_box;
pub use root_area::RootArea;
pub(crate) use root_box::{Measurement as RootMeasurement, record as record_root_box};

mod color_clip;
pub(crate) use color_clip::{ColorBox, color_layer, needs_color_layer, paint_color};
mod fills;
use crate::hooks::IntoHooked;
use fills::border_area_fill;
pub(crate) use fills::border_paint;
pub use fills::paint_area;

pub(crate) mod image_adapter;
mod source_types;
pub use crate::layout::replaced::natural::Intrinsic;
pub(crate) use crate::layout::replaced::natural::default_size;
pub use source_types::Source;

use crate::style::computed::Computed;
use gpui::{AnyElement, Bounds, IntoElement, Pixels, Styled, px};
pub(crate) mod alpha_sampling;
mod flat_vector;
mod oriented_vector;
mod sampling;
pub(crate) use sampling::paint_gradient;
mod mask_composite;
mod mask_luminance;
pub(crate) mod single_corner;
pub use mask_composite::{MaskLayer, compose_mask_layers};
pub(crate) use mask_luminance::luminance_mask_image;
mod clip;
mod conic;
mod gradient_raster;

pub(super) mod exact_layer;
mod radius_lengths;
mod tile_positions;
#[cfg(test)]
use crate::paint::background::tile_positions::tiling;
mod shape_path;
pub use crate::paint::background::shape_path::*;
mod shape_raster;
pub use crate::paint::background::shape_raster::*;
pub(super) mod raster;
pub(crate) use crate::paint::background::raster::*;
mod svg_background;
pub(crate) use svg_background::svg_root_background;
pub(super) mod tiles;
pub use crate::paint::background::tiles::*;

/// Потолок на число плиток вдоль оси: битый `background-size` иначе просит
/// миллионы копий.
const MAX_TILES: f32 = 2048.0;

/// Слой фона КАНВАСА: плитки меряются коробкой корня, а красят весь холст.
pub fn canvas_layer(c: &Computed, area: RootArea, vcx: ValueCx<'_>) -> Option<AnyElement> {
    let values = vcx.keep();
    c.bg_image.as_ref()?;
    let style = c.clone();
    Some(
        gpui::canvas(
            |_, _, _| {},
            move |clip: Bounds<Pixels>, _, window, _| {
                let vcx = values.vcx();
                let bounds = if style.bg_fixed == Some(true) {
                    clip
                } else {
                    area.rect(clip, vcx)
                };
                paint_tiles(&style, bounds, Some(clip), window, vcx);
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}

pub fn layer(c: &Computed, vcx: ValueCx<'_>) -> Option<AnyElement> {
    let values = vcx.keep();
    c.bg_image.as_ref()?;
    // Одноцветный фон `border-area` несёт краска рамки (`border_paint`):
    // плитки легли бы на всю коробку.
    if border_area_fill(c, vcx).is_some() {
        return None;
    }
    let style = c.clone();
    Some(
        exact_layer::ExactLayer::inset(
            c.bg_border_insets.unwrap_or([0.0; 4]),
            move |geometry: exact_layer::PaintGeometry, window: &mut gpui::Window| {
                let bounds = geometry.snapped;
                let vcx = values.vcx();
                clip::paint(&style, bounds, window, |style, window| {
                    if style.bg_fixed == Some(true) {
                        // Плитка меряется и отсчитывается от ОБЛАСТИ ПРОСМОТРА,
                        // а красится только внутри своей коробки: сдвиг между
                        // ними держит сам `paint_tiles`.
                        let view = Bounds {
                            origin: gpui::point(px(0.0), px(0.0)),
                            size: window.viewport_size(),
                        };
                        tiles::paint_tiles_in(
                            style,
                            view,
                            Some(bounds),
                            window,
                            Some(geometry),
                            vcx,
                        );
                    } else {
                        tiles::paint_tiles_in(style, bounds, None, window, Some(geometry), vcx);
                    }
                });
            },
        )
        .hooked(),
    )
}

// --- Сплошная заливка ----------------------------------------------------
//
// Фон, который сводится к ОДНОМУ цвету (цвет фона, одноцветный градиент,
// растр из одинаковых непрозрачных точек, мощённый без зазоров), не
// нуждается в маске для `background-clip: text | border-area`: области
// краски хватает цвета глифа или рамки.

pub(crate) use crate::style::values::composite::over;

#[cfg(test)]
mod tests;
