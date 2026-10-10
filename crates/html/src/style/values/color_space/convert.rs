//! Переходы между пространствами: Lab/OKLab <-> sRGB, HSL, отображение в охват sRGB.

use super::*;

pub(super) fn lab_to_srgb(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    // Lab → XYZ при точке белого D50 (§10.3).
    const K: f32 = 24389.0 / 27.0;
    const E: f32 = 216.0 / 24389.0;
    let fy = (l + 16.0) / 116.0;
    let fx = a / 500.0 + fy;
    let fz = fy - b / 200.0;
    let cube = |f: f32, _k: f32| {
        let c = f * f * f;
        if c > E { c } else { (116.0 * f - 16.0) / K }
    };
    let _ = K;
    let x = cube(fx, 0.0) * D50[0];
    let y = if l > K * E {
        fy * fy * fy * D50[1]
    } else {
        l / K * D50[1]
    };
    let z = cube(fz, 0.0) * D50[2];
    let xyz = mul(D50_TO_D65, [x, y, z]);
    let lin = mul(XYZ_TO_LINEAR_SRGB, xyz);
    (srgb_gamma(lin[0]), srgb_gamma(lin[1]), srgb_gamma(lin[2]))
}

pub(super) fn oklab_to_srgb(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
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
fn linear_srgb_to_oklab(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
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
pub(super) fn gamut_map(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
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

/// sRGB → OKLab: кривая снимается, дальше готовая матрица (§9.2).
pub(super) fn srgb_to_oklab(c: crate::style::values::value::Color) -> (f32, f32, f32) {
    linear_srgb_to_oklab(srgb_linear(c.r), srgb_linear(c.g), srgb_linear(c.b))
}

/// sRGB → CIE Lab при точке белого D50 (§10.3) — обратное к `lab_to_srgb`.
pub(super) fn srgb_to_lab(c: crate::style::values::value::Color) -> (f32, f32, f32) {
    const K: f32 = 24389.0 / 27.0;
    const E: f32 = 216.0 / 24389.0;
    let lin = [srgb_linear(c.r), srgb_linear(c.g), srgb_linear(c.b)];
    let xyz65 = mul(LINEAR_SRGB_TO_XYZ, lin);
    let xyz = mul(D65_TO_D50, xyz65);
    let f = |v: f32, w: f32| {
        let r = v / w;
        if r > E {
            r.cbrt()
        } else {
            (K * r + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(xyz[0], D50[0]), f(xyz[1], D50[1]), f(xyz[2], D50[2]));
    (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
}

/// Втянуть тройку в охват sRGB, не переписывая вызовы с кортежем.
pub(super) fn gamut_map_tuple(v: (f32, f32, f32)) -> (f32, f32, f32) {
    gamut_map(v.0, v.1, v.2)
}

/// sRGB → HSL: тон в градусах, насыщенность и светлота в долях.
pub(super) fn rgb_to_hsl(c: crate::style::values::value::Color) -> (f32, f32, f32) {
    let (max, min) = (c.r.max(c.g).max(c.b), c.r.min(c.g).min(c.b));
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - c.r).abs() < 1e-6 {
        ((c.g - c.b) / d).rem_euclid(6.0)
    } else if (max - c.g).abs() < 1e-6 {
        (c.b - c.r) / d + 2.0
    } else {
        (c.r - c.g) / d + 4.0
    };
    (h * 60.0, s, l)
}

/// HSL → sRGB.
pub(super) fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (r + m, g + m, b + m)
}
