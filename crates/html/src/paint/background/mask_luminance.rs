//! Luminance mask values quantized like an 8-bit compositor.
//!
//! CSS Masking 1 §7.10.1 ("Calculating mask values") applies the SVG 1.1
//! `luminanceToAlpha` coefficients (0.2125, 0.7154, 0.0721) to the colour and
//! multiplies by alpha. Gecko evaluates this on premultiplied 8-bit pixels
//! with integer weights 55/183/18 (sum 256) and `>> 8`
//! (SVGMaskFrame `ComputesRGBLuminanceMask`); the WPT mask-mode references
//! encode exactly those 8-bit results, e.g. blue -> 17/255.

use gpui::RenderImage;
use std::sync::Arc;

/// 8-bit luminance mask value of one straight BGRA pixel.
pub(crate) fn luminance8(b: u8, g: u8, r: u8, a: u8) -> u8 {
    if a == 0 {
        return 0;
    }
    let pm = |c: u8| (c as u32 * a as u32 + 127) / 255;
    ((55 * pm(r) + 183 * pm(g) + 18 * pm(b)) >> 8) as u8
}

/// Converts a mask layer image into an alpha-only mask carrying its
/// luminance values, so the compositor samples plain alpha.
pub(crate) fn luminance_mask_image(image: &Arc<RenderImage>) -> Option<Arc<RenderImage>> {
    let bytes = image.as_bytes(0)?;
    let size = image.size(0);
    let mut out = Vec::with_capacity(bytes.len());
    for px in bytes.chunks_exact(4) {
        let v = luminance8(px[0], px[1], px[2], px[3]);
        out.extend_from_slice(&[v, v, v, v]);
    }
    gpui::bgra_bytes_to_image(size.width.0 as u32, size.height.0 as u32, out)
}

#[cfg(test)]
mod tests {
    use super::luminance8;

    #[test]
    fn primaries_match_eight_bit_weights() {
        assert_eq!(luminance8(255, 0, 0, 255), 17);
        assert_eq!(luminance8(0, 0, 255, 255), 54);
        assert_eq!(luminance8(0, 255, 0, 255), 182);
        assert_eq!(luminance8(255, 255, 255, 255), 255);
        assert_eq!(luminance8(255, 255, 255, 0), 0);
    }
}
