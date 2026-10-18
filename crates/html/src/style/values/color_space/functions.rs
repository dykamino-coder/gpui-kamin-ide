//! Функция color() с предопределёнными пространствами и относительный синтаксис цвета (`rgb(from …)`).

use super::*;

/// `color(<пространство> c1 c2 c3[/A])` (§10).
pub(super) fn color_fn(body: &str) -> Option<(f32, f32, f32, f32)> {
    let (list, a) = parts(body);
    if list.len() < 4 {
        return None;
    }
    let space = list[0].to_ascii_lowercase();
    let c = [
        number(&list[1], 1.0)?,
        number(&list[2], 1.0)?,
        number(&list[3], 1.0)?,
    ];
    let lin = |v: [f32; 3]| [srgb_linear(v[0]), srgb_linear(v[1]), srgb_linear(v[2])];
    let xyz = match space.as_str() {
        "srgb" => {
            // Каналы за пределами [0,1] законны в записи — втягиваются тем
            // же сжатием цветности, что и остальные пространства.
            let (r, g, b) = gamut_map(c[0], c[1], c[2]);
            return Some((r, g, b, a));
        }
        "srgb-linear" => mul(LINEAR_SRGB_TO_XYZ, c),
        "display-p3" => mul(P3_TO_XYZ, lin(c)),
        // Линейный вариант: те же основные цвета, но без кривой.
        "display-p3-linear" => mul(P3_TO_XYZ, c),
        "rec2020-linear" => mul(REC2020_TO_XYZ, c),
        "a98-rgb-linear" => mul(A98_TO_XYZ, c),
        "prophoto-rgb-linear" => mul(D50_TO_D65, mul(PROPHOTO_TO_XYZ, c)),
        // У Adobe RGB своя степень кривой, простая и без прямого куска.
        "a98-rgb" => {
            let g = |v: f32| v.signum() * v.abs().powf(563.0 / 256.0);
            mul(A98_TO_XYZ, [g(c[0]), g(c[1]), g(c[2])])
        }
        "prophoto-rgb" => {
            let g = |v: f32| {
                let s = v.signum();
                let v = v.abs();
                s * if v <= 16.0 / 512.0 {
                    v / 16.0
                } else {
                    v.powf(1.8)
                }
            };
            // ProPhoto считается при D50 — переводим к D65.
            mul(
                D50_TO_D65,
                mul(PROPHOTO_TO_XYZ, [g(c[0]), g(c[1]), g(c[2])]),
            )
        }
        "rec2020" => {
            const A: f32 = 1.099_296_8;
            const B: f32 = 0.018_053_97;
            let g = |v: f32| {
                let s = v.signum();
                let v = v.abs();
                s * if v < B * 4.5 {
                    v / 4.5
                } else {
                    ((v + A - 1.0) / A).powf(1.0 / 0.45)
                }
            };
            mul(REC2020_TO_XYZ, [g(c[0]), g(c[1]), g(c[2])])
        }
        "xyz" | "xyz-d65" => c,
        "xyz-d50" => mul(D50_TO_D65, c),
        // Своё пространство из `@color-profile --имя { src: url(…) }`.
        custom if custom.starts_with("--") => {
            let profile = profile(custom)?;
            let (r, g, b) = icc_to_srgb(&profile, c)?;
            return Some((r, g, b, a));
        }
        _ => return None,
    };
    let out = mul(XYZ_TO_LINEAR_SRGB, xyz);
    // Втягивание в охват тем же сжатием цветности, что у lab/lch: пары
    // «lab против color(display-p3 …)» обязаны сходиться в ОДИН цвет sRGB.
    let (r, g, b) = gamut_map(srgb_gamma(out[0]), srgb_gamma(out[1]), srgb_gamma(out[2]));
    Some((r, g, b, a))
}

/// Resolve a specified relative color through the computed operand parser.
pub(crate) fn resolve_relative(
    expr: &str,
    current: crate::style::values::value::Color,
) -> Option<crate::style::values::value::Color> {
    match ColorExpression::parse(expr)? {
        ColorExpression::Color(_) => None,
        expression => Some(expression.resolve(current)),
    }
}
