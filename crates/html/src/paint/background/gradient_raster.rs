//! Адаптер CPU-градиентов для канваса GPUI.
use crate::values::ValueCx;
use std::sync::Arc;
pub(super) fn tile(
    src: &crate::style::computed::ImageSource,
    w: u32,
    h: u32,
    css: (f32, f32),
    vcx: ValueCx<'_>,
) -> Option<Arc<gpui::RenderImage>> {
    super::image_adapter::upload(&crate::resources::css_images::gradient_raster::tile(
        src, w, h, css, vcx,
    )?)
}
