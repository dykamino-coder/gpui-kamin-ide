//! Декодирование картинок фона в образ GPUI: EXIF-ориентация, SVG-размер, data:-URL,
//! собственный размер. Чтение метаданных (метка EXIF, размер SVG, тело `data:`) общее с
//! `resources`; здесь — прежний путь в `RenderImage` до перевода потребителей на
//! `resources::images`.

mod svg_metadata;
use svg_metadata::degenerate_viewbox;
pub(crate) use svg_metadata::svg_root_background;
pub(super) use svg_metadata::svg_size;

mod orientation;
use orientation::exif_orientation;
use orientation::orient_image;

use crate::paint::background::*;
#[cfg(test)]
pub(super) use crate::resources::loader::data_url::base64_decode;
pub(super) use crate::resources::loader::data_url::percent_decode;

/// Растр или рисунок — по содержимому файла, а не по расширению: у `data:`-URI
/// расширения нет вовсе.
pub(super) fn decode(bytes: &[u8], orient: bool) -> Option<Source> {
    if crate::resources::images::vector::looks_svg(bytes) {
        let markup = String::from_utf8(bytes.to_vec()).ok()?;
        // Вырожденная область просмотра (нулевая ось `viewBox`) — картинки
        // НЕТ вовсе (SVG intrinsic sizing): браузер такой фон не рисует.
        if degenerate_viewbox(&markup) {
            return None;
        }
        let size = svg_size(&markup);
        return Some(Source::Vector { markup, size });
    }
    let image = gpui::raster_bytes_to_image(bytes)?;
    // Вшитый цветовой профиль (PNG `iCCP`) — часть картинки: её точки заданы
    // в ЕГО пространстве (css-color-4 §12, tagged images).
    let image = match gpui::png_icc_profile(bytes)
        .and_then(|profile| crate::style::values::color_space::apply_icc(&image, &profile))
    {
        Some(fixed) => fixed,
        None => image,
    };
    // Разворот по EXIF (css-images-3 §5.4, начальное значение `from-image`):
    // «All CSS layout and rendering processes use the image AFTER rotation…
    // The natural height and width are derived from the rotated rather than
    // the original image dimensions». Значит применять надо ЗДЕСЬ, до того как
    // кто-нибудь спросит `Source::intrinsic()`, — как Blink разворачивает в
    // `LayoutImageResource::ImageOrientation`.
    if orient
        && let Some(tag) = exif_orientation(bytes)
        && tag > 1
        && let Some(turned) = orient_image(&image, tag)
    {
        return Some(Source::Raster(turned));
    }
    Some(Source::Raster(image))
}

pub(crate) fn read_bytes(src: &str) -> Option<Vec<u8>> {
    if let Some(rest) = src.strip_prefix("data:") {
        return crate::resources::loader::data_url::decode(rest);
    }
    let path = src.strip_prefix("file:///").unwrap_or(src);
    std::fs::read(path).ok()
}
