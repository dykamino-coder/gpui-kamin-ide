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
            let profile = PROFILES.with(|p| p.borrow().get(custom).cloned());
            let profile = profile?;
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

/// Относительный цвет (css-color-5 §4): `hsl(from currentColor h s l)`.
///
/// Тождественные каналы дают сам базовый цвет: преобразование туда-обратно в
/// плавающих пространствах без потерь. Подстановки каналов считаются честно
/// для `rgb()` и `hsl()` — прочим пространствам хватает тождества: обратные
/// преобразования им пока не заведены.
pub(crate) fn resolve_relative(
    expr: &str,
    current: crate::style::values::value::Color,
) -> Option<crate::style::values::value::Color> {
    use crate::style::values::value::Color;
    // Голое слово: цвет текста этого же элемента.
    if expr.eq_ignore_ascii_case("currentcolor") {
        return Some(current);
    }
    // `color-mix` с `currentColor`: слово подставляется уже решённым цветом,
    // дальше работает обычный разбор смеси.
    let low = expr.to_ascii_lowercase();
    if low.starts_with("color-mix(") {
        let rgb = format!(
            "rgb({} {} {} / {})",
            (current.r * 255.0).round(),
            (current.g * 255.0).round(),
            (current.b * 255.0).round(),
            current.a
        );
        return Color::parse(&low.replace("currentcolor", &rgb));
    }
    let open = expr.find('(')?;
    let name = expr[..open].trim();
    let inner = expr[open + 1..].trim().strip_suffix(')')?;
    let rest = inner.trim().strip_prefix("from ")?;
    let words = crate::style::computed::split_outside_parens(rest);
    let base_tok = words.first()?;
    let base = if base_tok.eq_ignore_ascii_case("currentcolor") {
        current
    } else {
        Color::parse(base_tok)?
    };
    // Прозрачность после косой: `alpha` — своя, число — новая.
    let mut channels: Vec<&str> = vec![];
    let mut alpha = base.a;
    let mut after_slash = false;
    for w in words[1..].iter().map(|w| w.as_str()) {
        if w == "/" {
            after_slash = true;
        } else if after_slash {
            alpha = match w {
                "alpha" => base.a,
                t => t
                    .strip_suffix('%')
                    .and_then(|n| n.parse::<f32>().ok().map(|v| v / 100.0))
                    .or_else(|| t.parse().ok())
                    .unwrap_or(base.a),
            };
        } else {
            channels.push(w);
        }
    }
    let identity: &[&str] = match name {
        "rgb" | "rgba" => &["r", "g", "b"],
        "hsl" | "hsla" => &["h", "s", "l"],
        "hwb" => &["h", "w", "b"],
        "lab" | "oklab" => &["l", "a", "b"],
        "lch" | "oklch" => &["l", "c", "h"],
        "color" => {
            // Первый канал — имя пространства.
            let space = channels.first().copied().unwrap_or("");
            let rest = &channels[1..];
            let names: &[&str] = if space.starts_with("xyz") {
                &["x", "y", "z"]
            } else {
                &["r", "g", "b"]
            };
            return (rest == names).then_some(Color { a: alpha, ..base });
        }
        _ => return None,
    };
    if channels == identity {
        return Some(Color { a: alpha, ..base });
    }
    match name {
        "rgb" | "rgba" => {
            let chan = |t: &str| match t {
                "r" => Some(base.r),
                "g" => Some(base.g),
                "b" => Some(base.b),
                t => t
                    .strip_suffix('%')
                    .and_then(|n| n.parse::<f32>().ok().map(|v| v / 100.0))
                    .or_else(|| t.parse::<f32>().ok().map(|v| v / 255.0)),
            };
            Some(Color {
                r: chan(channels.first()?)?.clamp(0.0, 1.0),
                g: chan(channels.get(1)?)?.clamp(0.0, 1.0),
                b: chan(channels.get(2)?)?.clamp(0.0, 1.0),
                a: alpha,
            })
        }
        "hsl" | "hsla" => {
            let (h, s, l) = rgb_to_hsl(base);
            let chan = |t: &str, own: f32, pct: bool| match t {
                "h" => Some(h),
                "s" => Some(s),
                "l" => Some(l),
                t => {
                    let _ = own;
                    if pct {
                        t.strip_suffix('%')
                            .and_then(|n| n.parse::<f32>().ok().map(|v| v / 100.0))
                            .or_else(|| t.parse().ok())
                    } else {
                        t.strip_suffix("deg").unwrap_or(t).parse::<f32>().ok()
                    }
                }
            };
            let (h, s, l) = (
                chan(channels.first()?, h, false)?,
                chan(channels.get(1)?, s, true)?.clamp(0.0, 1.0),
                chan(channels.get(2)?, l, true)?.clamp(0.0, 1.0),
            );
            let (r, g, b) = hsl_to_rgb(h, s, l);
            Some(Color { r, g, b, a: alpha })
        }
        _ => None,
    }
}
