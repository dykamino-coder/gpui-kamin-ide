//! Адаптер CPU-конусного градиента для канваса GPUI.
use super::Source;
use crate::document::state::ValueCx;
use crate::resources::css_images::Source as CpuSource;
use std::sync::Arc;
pub(super) fn tile(
    source: &Source,
    size: (f32, f32),
    density: f32,
    vcx: ValueCx<'_>,
) -> Option<Arc<gpui::RenderImage>> {
    if let Source::Image(CpuSource::Gradient { image }) = source
        && let crate::style::computed::ImageSource::Conic(g) = image.as_ref()
    {
        return super::image_adapter::upload(&crate::resources::css_images::conic::rasterize(
            g, size, density, vcx,
        )?);
    }
    source.raster(size, vcx)
}
