//! Адаптер пересэмплирования CPU-растра для графических слоёв покраски.

use std::sync::Arc;
pub(crate) fn resample(
    image: &Arc<gpui::RenderImage>,
    w: u32,
    h: u32,
) -> Option<Arc<gpui::RenderImage>> {
    let cpu = super::image_adapter::download(image)?;
    super::image_adapter::upload(&crate::resources::images::resample::resample(&cpu, w, h)?)
}
