//! Browser-compatible JPEG reconstruction, before CSS applies EXIF orientation.

use std::sync::Arc;
use turbojpeg::{Colorspace, Decompressor, Image, PixelFormat};

pub(super) fn decode(bytes: &[u8]) -> Option<Arc<gpui::RenderImage>> {
    let mut decoder = Decompressor::new().ok()?;
    // libjpeg-turbo defaults to accurate (ISLOW) IDCT. In particular, do not
    // enable FASTDCT or replace fancy 4:2:0/4:2:2 upsampling with replication.
    decoder.set_fast_upsample(false).ok()?;
    let header = decoder.read_header(bytes).ok()?;
    // TurboJPEG deliberately leaves CMYK color management to the caller.
    // Preserve the existing conversion for these uncommon four-channel files.
    if matches!(header.colorspace, Colorspace::CMYK | Colorspace::YCCK) {
        return gpui::raster_bytes_to_image(bytes);
    }
    let (width, height) = (
        u32::try_from(header.width).ok()?,
        u32::try_from(header.height).ok()?,
    );
    let pitch = header.width.checked_mul(4)?;
    let len = pitch.checked_mul(header.height)?;
    // Keep the image-rs decoded-image allocation limit used by the other
    // raster formats; the header must not dictate an unbounded allocation.
    if len > 512 * 1024 * 1024 {
        return None;
    }
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(len).ok()?;
    pixels.resize(len, 0);
    decoder
        .decompress(
            bytes,
            Image {
                pixels: pixels.as_mut_slice(),
                width: header.width,
                height: header.height,
                pitch,
                format: PixelFormat::BGRA,
            },
        )
        .ok()?;
    gpui::bgra_bytes_to_image(width, height, pixels)
}
