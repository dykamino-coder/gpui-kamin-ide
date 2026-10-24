//! Адаптер CPU-растеризации документа и кэш типизированных геометрических сцен.

use super::DENSITY;
use crate::paint::cache::{BoundedCache, MIB, optional_image_bytes};
use gpui::RenderImage;
use std::sync::Arc;

/// Типизированные геометрические сцены не зависят от ресурсов документа.
/// SVG-разметка идёт в отдельный кэш `ImageStore` с полной идентичностью.
static CACHE: BoundedCache<(u64, u32, u32), Option<Arc<RenderImage>>> =
    BoundedCache::new(128 * MIB, optional_image_bytes);

/// Растрировать рисунок. `None`, если разметка не разбирается — тогда
/// вызывающий покажет запасной текст, а не пустое место.
pub fn rasterize(
    markup: &str,
    w: f32,
    h: f32,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<Arc<RenderImage>> {
    let cpu = vcx.doc.images.borrow().rasterize_svg(
        markup,
        crate::resources::images::svg_raster::RasterRequest::new(w, h, DENSITY, 0.0),
    )?;
    crate::paint::background::image_adapter::upload(&cpu)
}

/// SVG как CSS/HTML-картинка: secure static mode запрещает внешние файлы.
/// <https://www.w3.org/TR/SVG/conform.html#secure-static-mode>
pub(crate) fn rasterize_image(
    markup: &str,
    w: f32,
    h: f32,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<Arc<RenderImage>> {
    let cpu = vcx.doc.images.borrow().rasterize_svg(
        markup,
        crate::resources::images::svg_raster::RasterRequest::new(w, h, DENSITY, 0.0)
            .without_external_resources(),
    )?;
    crate::paint::background::image_adapter::upload(&cpu)
}

/// Геометрическая декорация без документа: тот же CPU-бэкенд без кэша картинок.
pub(crate) fn geometry(markup: &str, w: f32, h: f32) -> Option<Arc<RenderImage>> {
    let cpu = crate::resources::images::svg_raster::rasterize(
        markup,
        crate::resources::images::svg_raster::RasterRequest::new(w, h, DENSITY, 0.0),
        Arc::new(resvg::usvg::fontdb::Database::new()),
        &|_| None,
        true,
    )?;
    crate::paint::background::image_adapter::upload(&cpu)
}

/// Device pixels of one raster axis, normalised exactly as gpui's SVG
/// rasterizer sizes its pixmap (`(logical * density).round().max(1)`).
///
/// The cache key must be the raster's real size: two requests that round to
/// the same logical pixel (10.1 and 10.4) still produce 20 and 21 device
/// pixels, while two that land on the same device size produce byte-identical
/// rasters and may share one entry.
pub(super) fn device_extent(logical: f32) -> u32 {
    (logical * DENSITY).round().max(1.0) as u32
}

/// Кэш типизированных декоративных сцен; разметка документа сюда не попадает.
/// `create` must rasterize at [`DENSITY`]: the key is its device-pixel size.
pub(super) fn cached(
    hash: u64,
    w: f32,
    h: f32,
    create: impl FnOnce() -> Option<Arc<RenderImage>>,
) -> Option<Arc<RenderImage>> {
    let key = (hash, device_extent(w), device_extent(h));
    CACHE.get_or_insert_with(key, create)
}

/// `rasterize` with `pad` logical pixels of extra canvas on the right and
/// bottom at the same user-space scale (`gpui::svg_markup_to_image_padded`).
pub(super) fn rasterize_padded(
    markup: &str,
    w: f32,
    h: f32,
    pad: f32,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<Arc<RenderImage>> {
    let cpu = vcx.doc.images.borrow().rasterize_svg(
        markup,
        crate::resources::images::svg_raster::RasterRequest::new(w, h, DENSITY, pad),
    )?;
    crate::paint::background::image_adapter::upload(&cpu)
}

#[cfg(test)]
mod tests {
    use super::geometry as rasterize;
    use super::{DENSITY, device_extent};

    const MARKUP: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'>                          <rect width='10' height='10' fill='green'/><!--cache-key--></svg>";

    #[test]
    fn cache_returns_raster_of_requested_device_size() {
        // 10.1 and 10.4 round to one logical pixel but to 20 and 21 device
        // pixels; the second request must not reuse the first raster.
        let a = rasterize(MARKUP, 10.1, 10.0).expect("a");
        let b = rasterize(MARKUP, 10.4, 10.0).expect("b");
        let direct = gpui::svg_markup_to_image(MARKUP, 10.4, 10.0, DENSITY).expect("direct");
        assert_eq!(a.size(0).width.0, 20);
        assert_eq!(b.size(0).width, direct.size(0).width);
        assert_eq!(b.size(0).width.0, 21);
    }

    #[test]
    fn device_extent_matches_gpui_pixmap_size() {
        for tenth in 0..60 {
            let w = tenth as f32 / 10.0;
            let image = gpui::svg_markup_to_image(MARKUP, w, 3.0, DENSITY).expect("raster");
            assert_eq!(image.size(0).width.0 as u32, device_extent(w), "width {w}");
        }
        assert_eq!(device_extent(f32::NAN), 1);
    }
}
