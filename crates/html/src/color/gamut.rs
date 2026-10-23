//! Перевод OKLab ↔ sRGB и втягивание цвета в охват sRGB сжатием цветности
//! (CSS Color 4 §9.2, §13.1.5): общая цветовая математика стиля и картинок.

use super::{srgb_gamma, srgb_linear};

pub(crate) fn oklab_to_srgb(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    // OKLab устроен так, что до линейного sRGB доходит без XYZ (§9.2).
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3;
    let b = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;
    (srgb_gamma(r), srgb_gamma(g), srgb_gamma(b))
}

/// Линейный sRGB (возможно, вне охвата) → OKLab.
pub(crate) fn linear_srgb_to_oklab(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let l = 0.412_221_47 * r + 0.536_332_54 * g + 0.051_445_995 * b;
    let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_84 * g + 0.629_978_7 * b;
    let (l_, m_, s_) = (l.cbrt(), m.cbrt(), s.cbrt());
    (
        0.210_454_26 * l_ + 0.793_617_8 * m_ - 0.004_072_047 * s_,
        1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_,
        0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_,
    )
}

/// Втягивание цвета в охват sRGB СЖАТИЕМ ЦВЕТНОСТИ (CSS Color 4 §13.1.5).
///
/// Каналы за пределами [0,1] нельзя просто отрезать: срез меняет и тон, и
/// светлоту (`lch(100% 110 60)` обязан стать БЕЛЫМ, а срез давал жёлтый).
/// Спекой задано уменьшение цветности в OKLCh до входа в охват; светлота с
/// краёв диапазона решается сразу.
pub(crate) fn gamut_map(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let eps = 1e-4;
    let inside = |v: (f32, f32, f32)| {
        (-eps..=1.0 + eps).contains(&v.0)
            && (-eps..=1.0 + eps).contains(&v.1)
            && (-eps..=1.0 + eps).contains(&v.2)
    };
    if inside((r, g, b)) {
        return (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0));
    }
    let (l, a, bb) = linear_srgb_to_oklab(srgb_linear(r), srgb_linear(g), srgb_linear(b));
    if l >= 1.0 {
        return (1.0, 1.0, 1.0);
    }
    if l <= 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let c0 = (a * a + bb * bb).sqrt();
    let h = bb.atan2(a);
    let (mut lo, mut hi) = (0.0f32, c0);
    for _ in 0..24 {
        let c = (lo + hi) / 2.0;
        let v = oklab_to_srgb(l, c * h.cos(), c * h.sin());
        if inside(v) {
            lo = c;
        } else {
            hi = c;
        }
    }
    let v = oklab_to_srgb(l, lo * h.cos(), lo * h.sin());
    (
        v.0.clamp(0.0, 1.0),
        v.1.clamp(0.0, 1.0),
        v.2.clamp(0.0, 1.0),
    )
}
