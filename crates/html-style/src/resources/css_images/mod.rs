//! CSS-изображения: вычисленные операнды, синхронный резолвер и CPU-растеризация.
//! Зависит от style и контекста документа; не зависит от paint, layout и GPUI.

pub mod conic;
pub mod facts;
pub mod filter_image;
mod filtered_raster;
pub mod flat_vector;
pub mod gradient_geometry;
pub mod gradient_raster;
mod presentation;
pub mod raster;
mod raster_keys;
mod source_types;
mod sources;
use crate::resources::images::natural::Intrinsic;
use crate::values::ValueCx;
use raster::{place_stops, rasterize_gradient, wrap_repeat};
pub use source_types::Source;
pub use sources::{computed_source, element_source, source};
/// Длина CSS-картинки: база по оси, шрифтовые единицы уже вычислены каскадом.
pub fn len_px(
    l: Option<crate::style::values::value::Len>,
    base: f32,
    vcx: ValueCx<'_>,
) -> Option<f32> {
    l?.to_px(Some(base), crate::style::computed::FontCx::FALLBACK, vcx)
}
