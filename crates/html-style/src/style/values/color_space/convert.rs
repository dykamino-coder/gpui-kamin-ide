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
