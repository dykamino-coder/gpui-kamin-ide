//! `line-height: normal` в пикселях по целопиксельным метрикам шрифта, как считает Blink:
//! подъём, спуск и зазор строк основного шрифта округляются порознь
//! (`SimpleFontData::PlatformInit`, `FontMetrics::AscentDescentWithHacks`) и складываются в
//! интерлиньяж. Доли регистрирует замер семейства; без них — доля × кегль. Здесь же высота
//! области содержимого полосы фона строчной коробки.

use crate::document::state::ValueCx;
use crate::tolerance::COORD_EPS;

pub(super) fn register(line: f32, ascent: f32, descent: f32, vcx: ValueCx<'_>) {
    if ascent <= 0.0 || descent < 0.0 {
        return;
    }
    let gap = (line - ascent - descent).max(0.0);
    vcx.doc
        .metrics
        .parts
        .borrow_mut()
        .insert(line.to_bits(), (ascent, descent, gap));
}

pub fn normal_px(fraction: f32, size: f32, vcx: ValueCx<'_>) -> f32 {
    let parts = vcx
        .doc
        .metrics
        .parts
        .borrow()
        .get(&fraction.to_bits())
        .copied();
    match parts {
        Some((a, d, g)) if size > 0.0 => {
            (a * size).round() + (d * size).round() + (g * size).round()
        }
        _ => fraction * size,
    }
}

/// The content area of an inline box's background band for the font whose
/// normal line fraction is `fraction`, at `size`: ascent + descent as the
/// glyphs are laid out, or, for a font with a line gap metric, the whole-pixel
/// normal line (the band GPUI paints, `paint_line_background`).
pub fn content_px(fraction: f32, size: f32, vcx: ValueCx<'_>) -> Option<f32> {
    vcx.doc
        .metrics
        .parts
        .borrow()
        .get(&fraction.to_bits())
        .copied()
        .map(|(a, d, g)| {
            if g * size > COORD_EPS {
                normal_px(fraction, size, vcx)
            } else {
                (a + d) * size
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_each_metric_separately() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        // Times New Roman: 1825 + 443 + 87 units of 2048.
        let (a, d, g) = (1825.0 / 2048.0, 443.0 / 2048.0, 87.0 / 2048.0);
        let line = a + d + g;
        register(line, a, d, vcx);
        assert_eq!(normal_px(line, 40.0, vcx), 47.0);
        assert_eq!(normal_px(line, 16.0, vcx), 18.0);
        assert_eq!(normal_px(1.5, 10.0, vcx), 15.0);
    }
}
