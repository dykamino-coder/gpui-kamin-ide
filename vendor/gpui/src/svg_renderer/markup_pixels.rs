//! Convert SVG markup texels with exact, nearest-byte unpremultiplication.

pub(super) fn swap_and_unpremultiply(pixel: &mut [u8]) {
    pixel.swap(0, 2);
    let alpha = u16::from(pixel[3]);
    if alpha == 0 {
        return;
    }
    // SVG raster bytes are premultiplied. The inverse must preserve a
    // saturated channel at every nonzero alpha: a/a is exactly one. Float
    // division followed by truncation turned white into 254 for many alpha
    // values, and CSS Masking 1 §7.10.1 then attenuated white luminance masks.
    // Round other channels to the nearest byte to minimize re-encoding error.
    for channel in &mut pixel[..3] {
        *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
    }
}
