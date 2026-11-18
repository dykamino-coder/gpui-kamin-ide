//! Единственная граница CPU ↔ GPUI: порядок каналов, альфа и готовые точки.
//! Размер CSS принадлежит потребителю; GPUI получает только точки устройства.
use crate::cache::{BoundedCache, MIB};
use crate::resources::images::{
    identity::RasterIdentity,
    pixels,
    types::{AlphaMode, CpuRaster, PixelFormat},
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

pub(crate) fn upload(cpu: &Arc<CpuRaster>) -> Image {
    IDENTITIES.get_or_insert_with(RasterIdentity(cpu.clone()), || {
        let dense = cpu.format == PixelFormat::Bgra8
            && cpu.alpha == AlphaMode::Straight
            && cpu.stride == cpu.width as usize * 4
            && cpu.bytes.len() == cpu.stride * cpu.height as usize;
        let ready = if dense {
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
