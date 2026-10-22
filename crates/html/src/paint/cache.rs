//! Вес растров GPUI для процессных кэшей покраски и `svg/`. Сам кэш с бюджетом в байтах
//! и LRU — нейтральный `crate::cache`; здесь — путь, по которому его берут фазы, и вес
//! `RenderImage`.

pub(crate) use crate::cache::{BoundedCache, Hinted, MIB, hint};
use gpui::RenderImage;
use std::sync::Arc;

/// Байты пикселей всех кадров растра.
pub(crate) fn image_bytes(image: &RenderImage) -> usize {
    (0..image.frame_count())
        .filter_map(|frame| image.as_bytes(frame))
        .map(<[u8]>::len)
        .sum()
}

/// Вес растра, которого может не быть (неразбираемый источник тоже запоминается).
pub(crate) fn optional_image_bytes(image: &Option<Arc<RenderImage>>) -> usize {
    image.as_deref().map_or(0, image_bytes)
}

/// Вес растра.
pub(crate) fn arc_image_bytes(image: &Arc<RenderImage>) -> usize {
    image_bytes(image)
}
