//! Единственная граница CPU ↔ GPUI: порядок каналов, альфа и готовые точки.
//! Размер CSS принадлежит потребителю; GPUI получает только точки устройства.
use crate::cache::{BoundedCache, MIB};
use crate::resources::images::{
    identity::RasterIdentity,
    pixels,
    types::{AlphaMode, CpuRaster, EncodedRaster, PixelFormat, RasterImage},
};
use std::sync::Arc;
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
/// by the atlas upload (of a painted image only) and not kept after it.
struct Lazy(Arc<EncodedRaster>);

impl gpui::LazyBgra for Lazy {
    fn bgra(&self) -> Option<Vec<u8>> {
        let cpu = self.0.pixels_once()?;
        Some(if dense(&cpu) {
            cpu.bytes.to_vec()
        } else {
            pixels::straight_bgra(&cpu)
        })
    }
}

/// Keyed by the raster's address: equal bytes are already merged by the decode cache.
#[derive(Clone)]
struct EncodedKey(Arc<EncodedRaster>);

impl PartialEq for EncodedKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for EncodedKey {}

impl std::hash::Hash for EncodedKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

// One GPUI image per file: its id keys the atlas texture across frames. The weight is
// the file bytes the key holds; an entry holds no pixels.
static ENCODED: BoundedCache<EncodedKey, Image> = BoundedCache::new(16 * MIB, |_| 0)
    .with_key_weight(|key: &EncodedKey| key.0.encoded_len())
    .for_paint();

pub(crate) fn upload_encoded(encoded: &Arc<EncodedRaster>) -> Image {
    ENCODED.get_or_insert_with(EncodedKey(encoded.clone()), || {
        let lazy = Arc::new(Lazy(encoded.clone()));
        gpui::RenderImage::from_lazy_bgra(encoded.width, encoded.height, lazy).map(Arc::new)
    })
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
        return lazy.0.pixels();
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
