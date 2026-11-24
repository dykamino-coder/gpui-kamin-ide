//! Единственная граница CPU ↔ GPUI: порядок каналов, альфа и готовые точки.
//! Размер CSS принадлежит потребителю; GPUI получает только точки устройства.
use crate::cache::{BoundedCache, MIB};
use crate::resources::images::{
    identity::RasterIdentity,
    pixels,
    types::{AlphaMode, CpuRaster, EncodedRaster, PixelFormat, RasterImage},
};
use std::sync::{Arc, Weak};
type Image = Option<Arc<gpui::RenderImage>>;
type UploadedPixels = (u32, u32, Arc<[u8]>);

// Два уровня делят прежний бюджет 128 МиБ. Быстрый путь не хэширует точки
// повторно в каждом кадре; ключ удерживает Arc от повторного адреса.
static IDENTITIES: BoundedCache<RasterIdentity, Image> =
    BoundedCache::new(64 * MIB, crate::paint::cache::optional_image_bytes)
        .with_key_weight(|key: &RasterIdentity| key.0.bytes.len())
        .for_paint();
// Только готовые точки: ни URL, ни политика, ни ресурсы документа здесь не
// переиспользуются. Полное равенство байт исключает коллизии короткого хэша.
static PIXELS: BoundedCache<UploadedPixels, Image> =
    BoundedCache::new(64 * MIB, crate::paint::cache::optional_image_bytes)
        .with_key_weight(|key: &UploadedPixels| key.2.len())
        .for_paint();

/// An image file for a texture: GPUI gets its size at once, and the pixels are decoded
/// by the atlas upload (of a painted image only) and not kept after it. The file is held
/// weakly: the image is the file's own upload (`EncodedRaster::upload`).
struct Lazy(Weak<EncodedRaster>);

impl gpui::LazyBgra for Lazy {
    fn bgra(&self) -> Option<Vec<u8>> {
        self.0.upgrade()?.bgra_once()
    }
}

/// One GPUI image per file: its id keys the atlas texture across frames, and it lives
/// exactly as long as the file's raster (held by the documents and the decode cache),
/// never longer: a process cache of it weighed by file bytes once pinned textures of
/// closed documents (ishadeed: +22 textures per document).
pub(crate) fn upload_encoded(encoded: &Arc<EncodedRaster>) -> Image {
    let image = encoded.upload(|| {
        let lazy = Arc::new(Lazy(Arc::downgrade(encoded)));
        gpui::RenderImage::from_lazy_bgra(encoded.width, encoded.height, lazy).map(Arc::new)
    })?;
    (*image).clone()
}

/// The raster of an image for a texture: a file lazily, ready pixels as they are.
pub(crate) fn upload_image(image: &RasterImage) -> Image {
    match image {
        RasterImage::Ready(cpu) => upload(cpu),
        RasterImage::Encoded(encoded) => upload_encoded(encoded),
    }
}

fn dense(cpu: &CpuRaster) -> bool {
    cpu.format == PixelFormat::Bgra8
        && cpu.alpha == AlphaMode::Straight
        && cpu.stride == cpu.width as usize * 4
        && cpu.bytes.len() == cpu.stride * cpu.height as usize
}
pub(crate) fn upload(cpu: &Arc<CpuRaster>) -> Image {
    crate::work::time!(_upload, PaintUploadNs);
    IDENTITIES.get_or_insert_with(RasterIdentity(cpu.clone()), || {
        let ready = if dense(cpu) {
            cpu.clone()
        } else {
            pixels::bgra(cpu.width, cpu.height, pixels::straight_bgra(cpu))?
        };
        let key: UploadedPixels = (ready.width, ready.height, ready.bytes.clone());
        PIXELS.get_or_insert_with(key.clone(), || {
            gpui::RenderImage::from_shared_bgra(key.0, key.1, ready).map(Arc::new)
        })
    })
}
// ImageId выдаётся GPUI монотонно; переиспользования адреса Arc в ключе нет.
static DOWNLOADS: BoundedCache<gpui::ImageId, Option<Arc<CpuRaster>>> =
    BoundedCache::new(128 * MIB, pixels::optional_bytes).for_paint();
pub(crate) fn download(image: &gpui::RenderImage) -> Option<Arc<CpuRaster>> {
    if let Some(cpu) = image.shared_bgra_owner::<CpuRaster>() {
        return Some(cpu);
    }
    if let Some(lazy) = image.shared_bgra_owner::<Lazy>() {
        return lazy.0.upgrade()?.pixels();
    }
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
mod tests;
