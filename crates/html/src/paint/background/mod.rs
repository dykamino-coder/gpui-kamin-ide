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

mod root_area;
pub use root_area::RootArea;

mod fills;
use fills::border_area_fill;
pub(crate) use fills::border_paint;
pub use fills::paint_area;
pub(crate) use fills::text_clip_fill;

mod source_types;
pub use source_types::Source;

use crate::style::computed::Computed;
use gpui::{AnyElement, Bounds, IntoElement, Pixels, RenderImage, Styled, px};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
pub(crate) mod alpha_sampling;
mod float_geometry;
mod oriented_vector;
mod sampling;
use crate::paint::background::float_geometry::rrect_of;
pub use float_geometry::rounded_float;
mod mask_composite;
pub use mask_composite::{MaskLayer, compose_mask_layers};
mod conic;
mod sources;
pub use sources::{key, key_exif, source};
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
pub fn load(src: &str) -> Option<Arc<RenderImage>> {
    match source(src)? {
        Source::Raster(image) => Some(image),
        // Своя величина рисунка — то же, что для растра: он растрируется под
        // неё, а нужный размер плитки посчитает вызывающий.
        Source::Vector { markup, size } => {
            let (w, h) = default_size(size, (300.0, 150.0));
            crate::svg::raster::rasterize(&markup, w, h)
        }
        Source::Gradient { raw } => rasterize_gradient(&raw, 300, 150),
        Source::Shape { raw } => rasterize_shape(&raw, 300, 150, 1.0),
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
pub fn intrinsic_px(src: &str) -> Option<(f32, f32)> {
    Some(default_size(source(src)?.intrinsic(), (300.0, 150.0)))
}

/// Разбить запись по пробелам ВЕРХНЕГО уровня: `calc(10px + 15%)` — один
/// токен. Без скобок — ровно `split_whitespace`.
pub(crate) fn split_top(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start: Option<usize> = None;
    for (i, ch) in s.char_indices() {
        if ch.is_whitespace() && depth <= 0 {
            if let Some(st) = start.take() {
                out.push(&s[st..i]);
            }
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        start.get_or_insert(i);
    }
    if let Some(st) = start {
        out.push(&s[st..]);
    }
    out
}

/// Слой фона КАНВАСА: плитки меряются коробкой корня, а красят весь холст.
pub fn canvas_layer(c: &Computed, area: RootArea) -> Option<AnyElement> {
    c.bg_image.as_ref()?;
    let style = c.clone();
    Some(
        gpui::canvas(
            |_, _, _| {},
            move |clip: Bounds<Pixels>, _, window, _| {
                paint_tiles(&style, area.rect(clip), Some(clip), window);
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}

pub fn layer(c: &Computed) -> Option<AnyElement> {
    c.bg_image.as_ref()?;
    // Одноцветный фон `border-area` несёт краска рамки (`border_paint`):
    // плитки легли бы на всю коробку.
    if border_area_fill(c).is_some() {
        return None;
    }
    let style = c.clone();
    Some(
        exact_layer::ExactLayer::new(move |bounds: Bounds<Pixels>, window: &mut gpui::Window| {
            if style.bg_fixed == Some(true) {
                // Плитка меряется и отсчитывается от ОБЛАСТИ ПРОСМОТРА,
                // а красится только внутри своей коробки: сдвиг между
                // ними держит сам `paint_tiles`.
                let view = Bounds {
                    origin: gpui::point(px(0.0), px(0.0)),
                    size: window.viewport_size(),
                };
                paint_tiles(&style, view, Some(bounds), window);
            } else {
                paint_area(&style, bounds, window);
            }
        })
        .into_any_element(),
    )
}

// --- Сплошная заливка ----------------------------------------------------
//
// Фон, который сводится к ОДНОМУ цвету (цвет фона, одноцветный градиент,
// растр из одинаковых непрозрачных точек, мощённый без зазоров), не
// нуждается в маске для `background-clip: text | border-area`: области
// краски хватает цвета глифа или рамки.

/// Краска `top` поверх `base` (source-over), цвета без премультипликации.
pub(crate) fn over(
    top: crate::style::values::value::Color,
    base: crate::style::values::value::Color,
) -> crate::style::values::value::Color {
    let a = top.a + base.a * (1.0 - top.a);
    if a <= 0.0 {
        return crate::style::values::value::Color::default();
    }
    let mix = |t: f32, b: f32| (t * top.a + b * base.a * (1.0 - top.a)) / a;
    crate::style::values::value::Color {
        r: mix(top.r, base.r),
        g: mix(top.g, base.g),
        b: mix(top.b, base.b),
        a,
    }
}

#[cfg(test)]
mod tests;
