//! Вшитый цветовой профиль картинки (css-color-4 §12, tagged images): извлечение из PNG
//! `iCCP` и перевод точек CPU-растра в sRGB через общую цветовую математику (`color`).
//! Профиль трансформирует только RGB; альфа не меняется.

use crate::color::{IccProfile, clamp_near_gamut};

/// Профиль ICC из куска `iCCP` PNG; у прочих форматов и без куска — `None`.
///
/// Только PNG — как у прежнего пути покраски (`gpui::png_icc_profile`); профили JPEG и
/// WebP — отдельное решение с проверкой WPT, а не побочный эффект переноса.
pub(crate) fn png_icc_profile(bytes: &[u8]) -> Option<Vec<u8>> {
    use image::ImageDecoder;

    let cursor = std::io::Cursor::new(bytes);
    let mut decoder = image::codecs::png::PngDecoder::new(cursor).ok()?;
    decoder.icc_profile().ok().flatten()
}

/// Перевести плотные точки RGBA с прямой альфой из пространства профиля в sRGB.
pub(crate) fn convert_to_srgb(rgba: &mut [u8], profile: &IccProfile) {
    for px in rgba.chunks_exact_mut(4) {
        let c = [
            px[0] as f32 / 255.0,
            px[1] as f32 / 255.0,
            px[2] as f32 / 255.0,
        ];
        let (r, g, b) = clamp_near_gamut(profile.to_srgb_unmapped(c));
        px[0] = (r * 255.0).round() as u8;
        px[1] = (g * 255.0).round() as u8;
        px[2] = (b * 255.0).round() as u8;
    }
}
