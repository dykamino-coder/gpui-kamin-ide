//! Ориентация декодированного растра GPUI по EXIF: метку читает и точки переставляет
//! `resources::images::orientation`, здесь — только обёртка в `RenderImage`.

use gpui::RenderImage;
use std::sync::Arc;

pub(super) use crate::resources::images::orientation::exif_orientation;

/// Развернуть растр по метке EXIF (TIFF 6.0, значения 1..8).
///
/// Точки переставляются целыми четвёрками — порядок каналов (BGRA,
/// премультиплицированный) при этом не важен, как и в `crop_image`.
pub(super) fn orient_image(image: &Arc<RenderImage>, tag: u16) -> Option<Arc<RenderImage>> {
    let s = image.size(0);
    let (w, h) = (s.width.0 as u32, s.height.0 as u32);
    let bytes = image.as_bytes(0)?;
    let (ow, oh, out) = crate::resources::images::orientation::orient_pixels(w, h, bytes, tag)?;
    gpui::bgra_bytes_to_image(ow, oh, out)
}
