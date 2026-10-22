//! Плотный CPU-растр BGRA и явное преобразование порядка каналов и альфы.

use super::types::{AlphaMode, ColourSpace, CpuRaster, PixelFormat};
use std::sync::Arc;

pub(crate) fn bgra(w: u32, h: u32, bytes: Vec<u8>) -> Option<Arc<CpuRaster>> {
    let stride = (w as usize).checked_mul(4)?;
    if stride.checked_mul(h as usize)? != bytes.len() {
        return None;
    }
    Some(Arc::new(CpuRaster {
        width: w,
        height: h,
        stride,
        format: PixelFormat::Bgra8,
        alpha: AlphaMode::Straight,
        colour_space: ColourSpace::Srgb,
        bytes: bytes.into(),
    }))
}

/// Прямые точки BGRA для фильтров и загрузки в графический адаптер.
pub(crate) fn straight_bgra(raster: &CpuRaster) -> Vec<u8> {
    let mut out = Vec::with_capacity(raster.width as usize * raster.height as usize * 4);
    for row in raster
        .bytes
        .chunks_exact(raster.stride)
        .take(raster.height as usize)
    {
        for pixel in row[..raster.width as usize * 4].chunks_exact(4) {
            let mut p = [pixel[0], pixel[1], pixel[2], pixel[3]];
            if raster.format == PixelFormat::Rgba8 {
                p.swap(0, 2);
            }
            if raster.alpha == AlphaMode::Premultiplied && p[3] != 0 {
                let a = p[3] as f32 / 255.0;
                for c in &mut p[..3] {
                    *c = (*c as f32 / a) as u8;
                }
            }
            out.extend_from_slice(&p);
        }
    }
    out
}

pub(crate) fn optional_bytes(image: &Option<Arc<CpuRaster>>) -> usize {
    image.as_ref().map_or(0, |r| r.bytes.len())
}
