//! Значения маски по светимости, квантованные как в 8-битном композиторе: перевод картинки
//! слоя маски в альфа-маску с её светимостью, чтобы композитор сэмплировал обычную альфу.
//!
//! CSS Masking 1 §7.10.1 («Calculating mask values») применяет к цвету коэффициенты
//! `luminanceToAlpha` из SVG 1.1 (0.2125, 0.7154, 0.0721) и умножает на альфу. Gecko считает
//! это на премультиплицированных 8-битных точках целыми весами 55/183/18 (сумма 256) и `>> 8`
//! (SVGMaskFrame `ComputesRGBLuminanceMask`); эталоны WPT mask-mode закодированы ровно этими
//! 8-битными результатами, например синий -> 17/255.

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
