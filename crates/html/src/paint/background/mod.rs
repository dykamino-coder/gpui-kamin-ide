//! Фоновая картинка: `background-image: url(...)` со всей её механикой.
//!
//! Почему отдельным проходом, а не элементом `img`. Фон в CSS — это заливка:
//! она мостится плитками, смещается, масштабируется и обрезается по коробке,
//! причём независимо от содержимого элемента. Элемент-картинка так не умеет:
//! он рисует ровно одну копию и участвует в раскладке. Поэтому фон рисуется
//! канвасом, который знает свои границы во время отрисовки, и кладёт нужное
//! число копий сам.
//!
//! Образ декодируется один раз и лежит в кэше: разбор PNG на каждом кадре
//! стоил бы дороже всей остальной отрисовки документа.

use crate::document::state::ValueCx;
mod root_area;
mod root_box;
pub use root_area::RootArea;
pub(crate) use root_box::{Measurement as RootMeasurement, record as record_root_box};

mod fills;
use crate::hooks::IntoHooked;
use fills::border_area_fill;
pub(crate) use fills::border_paint;
pub use fills::paint_area;

mod filtered_raster;
mod source_types;
pub use source_types::Source;

use crate::style::computed::Computed;
use gpui::{AnyElement, Bounds, IntoElement, Pixels, RenderImage, Styled, px};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
pub(crate) mod alpha_sampling;
mod flat_vector;
mod float_geometry;
mod oriented_vector;
mod sampling;
pub(crate) use sampling::paint_gradient;
pub(crate) mod single_corner;
use crate::paint::background::float_geometry::rrect_of;
pub use float_geometry::rounded_float;
pub(crate) mod filter_image;
mod mask_composite;
mod mask_luminance;
pub use mask_composite::{MaskLayer, compose_mask_layers};
pub(crate) use mask_luminance::luminance_mask_image;
mod angle;
mod clip;
mod conic;
mod sources;
pub use sources::{key, key_exif, source};
mod gradient_geometry;
mod gradient_raster;
mod svg_fragment;

pub(super) mod exact_layer;
mod radius_lengths;
mod tile_positions;
#[cfg(test)]
use crate::paint::background::tile_positions::tiling;
mod shape_path;
pub use crate::paint::background::shape_path::*;
mod shape_raster;
pub use crate::paint::background::shape_raster::*;
pub(super) mod shape_profile;
pub use crate::paint::background::shape_profile::*;
pub(super) mod raster;
pub(crate) use crate::paint::background::raster::*;
pub(super) mod image_decode;
pub use crate::paint::background::image_decode::*;
pub(super) mod tiles;
pub use crate::paint::background::tiles::*;

type Cache = Mutex<HashMap<String, Option<Source>>>;
static CACHE: OnceLock<Cache> = OnceLock::new();

/// Сколько разных картинок держим декодированными.
const CACHE_CAP: usize = 32;

/// Потолок на число плиток вдоль оси: битый `background-size` иначе просит
/// миллионы копий.
const MAX_TILES: f32 = 2048.0;

/// Декодировать по ссылке из `url(...)`: `data:`-URI или путь на диске.
///
/// Сеть не трогаем по тем же причинам, что и в элементе-картинке: документ
/// рисуется в чате, где загрузка чужих адресов недопустима.
pub fn load(src: &str, vcx: ValueCx<'_>) -> Option<Arc<RenderImage>> {
    match source(src, vcx)? {
        Source::Raster(image) => Some(image),
        filtered @ Source::Filtered { .. } => {
            filtered.raster(default_size(filtered.intrinsic(), (300.0, 150.0)), vcx)
        }
        // Своя величина рисунка — то же, что для растра: он растрируется под
        // неё, а нужный размер плитки посчитает вызывающий.
        Source::Vector { markup, size } => {
            let (w, h) = default_size(size, (300.0, 150.0));
            crate::svg::raster::rasterize(&markup, w, h)
        }
        Source::Gradient { raw } => rasterize_gradient(&raw, 300, 150, vcx),
        Source::Shape { raw } => rasterize_shape(&raw, 300, 150, 1.0, vcx),
    }
}

/// Своя величина картинки в CSS-точках (css-images-3 §5.1, недостающее —
/// §5.3 от места 300×150, как у `load`).
///
/// Размер растра `load` для этого не годится: рисунок растрируется с
/// плотностью `svg::DENSITY` (2×), и флоат-картинка с формой из 100-точечного
/// SVG заводилась 200×200 (`shape-image-002`: вырез на всю ширину
/// контейнера, `-017`: контейнер вдвое выше). У растра величина и есть его
/// точки — там ничего не меняется.
pub fn intrinsic_px(src: &str, vcx: ValueCx<'_>) -> Option<(f32, f32)> {
    Some(default_size(source(src, vcx)?.intrinsic(), (300.0, 150.0)))
}

pub(crate) use crate::style::css::value_tokens::split_top;

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
