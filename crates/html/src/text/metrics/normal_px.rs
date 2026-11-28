//! Normal line height uses device-pixel font metrics, returned in CSS coordinates.
//! подъём, спуск и зазор строк основного шрифта округляются порознь
//! (`SimpleFontData::PlatformInit`, `FontMetrics::AscentDescentWithHacks`) и складываются в
//! интерлиньяж. Доли регистрирует замер семейства; без них — доля × кегль. Здесь же высота
//! области содержимого полосы фона строчной коробки.

use crate::text::text_fonts::TextFonts;
use crate::tolerance::COORD_EPS;

pub(crate) fn register(line: f32, ascent: f32, descent: f32, fonts: &TextFonts) {
    if ascent <= 0.0 || descent < 0.0 {
        return;
    }
    let gap = (line - ascent - descent).max(0.0);
    fonts
        .metrics
        .parts
        .borrow_mut()
        .insert(line.to_bits(), (ascent, descent, gap));
}

pub(crate) fn normal_px(fraction: f32, size: f32, fonts: &TextFonts) -> f32 {
    let parts = fonts
        .metrics
        .parts
        .borrow()
        .get(&fraction.to_bits())
        .copied();
    match parts {
        Some((a, d, g)) if size > 0.0 => {
            let scale = fonts.device_scale();
            used_metric(a * size, scale)
                + used_metric(d * size, scale)
                + used_metric(g * size, scale)
        }
        _ => fraction * size,
    }
}

/// Quantize one metric on the device grid, then return to CSS coordinates.
pub(crate) fn used_metric(value: f32, scale: f32) -> f32 {
    (value * scale).round() / scale
}

/// Quantize the start half-leading; the end retains the remainder and the exact height.
pub(crate) fn used_baseline(height: f32, ascent: f32, descent: f32, scale: f32) -> f32 {
    ((height * scale - ascent * scale - descent * scale) / 2.0).floor() / scale + ascent
}

/// The content area of an inline box's background band for the font whose
/// normal line fraction is `fraction`, at `size`: ascent + descent as the
/// glyphs are laid out, or, for a font with a line gap metric, the whole-pixel
/// normal line (the band GPUI paints, `paint_line_background`).
pub(crate) fn content_px(fraction: f32, size: f32, fonts: &TextFonts) -> Option<f32> {
    fonts
        .metrics
        .parts
        .borrow()
        .get(&fraction.to_bits())
        .copied()
        .map(|(a, d, g)| {
            if g * size > COORD_EPS {
                normal_px(fraction, size, fonts)
            } else {
                let scale = fonts.device_scale();
                used_metric(a * size, scale) + used_metric(d * size, scale)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_each_metric_separately() {
        let doc = crate::document::state::DocState::new(Default::default());
        let fonts = &*doc.text;
        // Times New Roman: 1825 + 443 + 87 units of 2048.
        let (a, d, g) = (1825.0 / 2048.0, 443.0 / 2048.0, 87.0 / 2048.0);
        let line = a + d + g;
        register(line, a, d, fonts);
        assert_eq!(normal_px(line, 40.0, fonts), 47.0);
        assert_eq!(normal_px(line, 16.0, fonts), 18.0);
        assert_eq!(normal_px(1.5, 10.0, fonts), 15.0);
    }

    #[test]
    fn device_grid_changes_normal_and_content_without_rounding_explicit_heights() {
        let doc = crate::document::state::DocState::new(Default::default());
        let fonts = &*doc.text;
        let (a, d, g) = (0.90527344, 0.21191406, 0.032714844);
        let line = a + d + g;
        register(line, a, d, fonts);
        assert_eq!(normal_px(line, 16.0, fonts), 18.0);
        let generation = doc.values().doc.font_generation();
        fonts.set_device_scale(1.25, doc.values());
        assert_ne!(doc.values().doc.font_generation(), generation);
        let generation = doc.values().doc.font_generation();
        fonts.set_device_scale(1.25, doc.values());
        assert_eq!(doc.values().doc.font_generation(), generation);
        assert_eq!(normal_px(line, 16.0, fonts), 18.4);
        assert_eq!(normal_px(1.5, 10.0, fonts), 15.0);
        register(a + d, a, d, fonts);
        assert_eq!(content_px(a + d, 16.0, fonts), Some(17.6));
        fonts.set_device_scale(1.0, doc.values());
        assert_eq!(normal_px(line, 16.0, fonts), 18.0);
    }
}
