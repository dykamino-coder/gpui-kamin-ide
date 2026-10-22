//! Единственная граница CPU ↔ GPUI: порядок каналов, альфа и готовые точки.
//! Размер CSS принадлежит потребителю; GPUI получает только точки устройства.
use crate::cache::{BoundedCache, MIB};
use crate::resources::images::{identity::RasterIdentity, pixels, types::CpuRaster};
use std::sync::Arc;
static CACHE: BoundedCache<RasterIdentity, Option<Arc<gpui::RenderImage>>> =
    BoundedCache::new(128 * MIB, crate::paint::cache::optional_image_bytes)
        .with_key_weight(|key: &RasterIdentity| key.0.bytes.len());
pub(crate) fn upload(cpu: &Arc<CpuRaster>) -> Option<Arc<gpui::RenderImage>> {
    CACHE.get_or_insert_with(RasterIdentity(cpu.clone()), || {
        gpui::bgra_bytes_to_image(cpu.width, cpu.height, pixels::straight_bgra(cpu))
    })
}
// ImageId выдаётся GPUI монотонно; переиспользования адреса Arc в ключе нет.
static DOWNLOADS: BoundedCache<gpui::ImageId, Option<Arc<CpuRaster>>> =
    BoundedCache::new(128 * MIB, pixels::optional_bytes);
pub(crate) fn download(image: &gpui::RenderImage) -> Option<Arc<CpuRaster>> {
    DOWNLOADS.get_or_insert_with(image.id, || {
        let size = image.size(0);
        pixels::bgra(
            size.width.0 as u32,
            size.height.0 as u32,
            image.as_bytes(0)?.to_vec(),
        )
    })
}
#[cfg(test)]
mod tests {
    use super::upload;
    use crate::resources::images::{fixtures, types::*};
    use std::sync::Arc;

    #[test]
    fn straight_and_premultiplied_match_legacy_upload() {
        let rgba = vec![8, 16, 24, 128, 7, 9, 11, 0, 3, 5, 7, 255];
        for alpha in [AlphaMode::Straight, AlphaMode::Premultiplied] {
            let cpu = Arc::new(CpuRaster {
                width: 3,
                height: 1,
                stride: 12,
                format: PixelFormat::Rgba8,
                alpha,
                colour_space: ColourSpace::Srgb,
                bytes: rgba.clone().into(),
            });
            let mut expected = rgba.clone();
            for p in expected.chunks_exact_mut(4) {
                if alpha == AlphaMode::Premultiplied {
                    gpui::swap_rgba_pa_to_bgra(p);
                } else {
                    p.swap(0, 2);
                }
            }
            let image = upload(&cpu).unwrap();
            assert_eq!(image.as_bytes(0).unwrap(), expected);
            assert!(Arc::ptr_eq(&image, &upload(&cpu).unwrap()));
        }
    }

    #[test]
    fn translucent_png_icc_golden() {
        let png = fixtures::with_chunk(
            &fixtures::solid_png([8, 16, 24, 128]),
            b"iCCP",
            &fixtures::iccp(&fixtures::linear_srgb_profile()),
        );
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let source = crate::resources::css_images::source(
            &fixtures::data_url("image/png", &png),
            doc.values(),
        )
        .unwrap();
        let image = upload(&source.raster((1.0, 1.0), doc.values()).unwrap()).unwrap();
        assert_eq!(image.as_bytes(0).unwrap(), [82, 68, 48, 128]);
    }

    #[test]
    fn svg_cpu_matches_previous_gpui_rasterizer() {
        let store = crate::resources::images::store::ImageStore::default();
        for body in [
            "<rect width='9' height='7' fill='#214365' opacity='.37'/>",
            "<circle cx='4.5' cy='3.5' r='3' fill='red'/>",
            "<rect width='11' height='9' fill='#214365' opacity='.37'/>",
            "<defs><linearGradient id='g'><stop stop-color='red'/><stop offset='1' stop-color='blue' stop-opacity='.3'/></linearGradient></defs><rect width='9' height='7' fill='url(#g)'/>",
            "",
        ] {
            let markup = format!(
                "<svg xmlns='http://www.w3.org/2000/svg' width='9' height='7'>{body}</svg>"
            );
            for (w, h, density) in [(9.0, 7.0, 1.0), (10.4, 6.7, 2.0), (3.0, 5.0, 1.5)] {
                let old = gpui::svg_markup_to_image(&markup, w, h, density).unwrap();
                let cpu = store
                    .rasterize_svg(
                        &markup,
                        crate::resources::images::svg_raster::RasterRequest::new(
                            w, h, density, 0.0,
                        ),
                    )
                    .unwrap();
                assert_eq!(
                    upload(&cpu).unwrap().as_bytes(0),
                    old.as_bytes(0),
                    "{body} {density}"
                );
                let old = gpui::svg_markup_to_image_padded(&markup, w, h, density, 1.5);
                let cpu = store.rasterize_svg(
                    &markup,
                    crate::resources::images::svg_raster::RasterRequest::new(w, h, density, 1.5),
                );
                assert_eq!(
                    cpu.as_ref()
                        .and_then(upload)
                        .and_then(|image| image.as_bytes(0).map(<[u8]>::to_vec)),
                    old.as_ref()
                        .and_then(|image| image.as_bytes(0).map(<[u8]>::to_vec)),
                    "padded {body} {density}"
                );
            }
        }
    }
}
