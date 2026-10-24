//! CSS-изображения: вычисленные операнды, синхронный резолвер и CPU-растеризация.
//! Зависит от style и контекста документа; не зависит от paint, layout и GPUI.

pub(crate) mod conic;
pub(crate) mod facts;
pub(crate) mod filter_image;
mod filtered_raster;
pub(crate) mod flat_vector;
pub(crate) mod gradient_geometry;
pub(crate) mod gradient_raster;
mod presentation;
pub(crate) mod raster;
mod raster_keys;
mod source_types;
mod sources;
use crate::document::state::ValueCx;
use crate::resources::images::natural::Intrinsic;
use raster::{place_stops, rasterize_gradient, wrap_repeat};
pub(crate) use source_types::Source;
pub(crate) use sources::{computed_source, element_source, source};
fn len_px(l: Option<crate::style::values::value::Len>, base: f32, vcx: ValueCx<'_>) -> Option<f32> {
    l?.to_px(Some(base), crate::style::computed::FontCx::FALLBACK, vcx)
}
