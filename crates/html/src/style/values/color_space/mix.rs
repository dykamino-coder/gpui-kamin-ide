//! color-mix(): смешение в прямоугольных и полярных пространствах, интерполяция оттенка.

use super::*;

/// `color-mix(in <пространство> [<дуга> hue]?, <цвет> <доля>?, <цвет> <доля>?)`
/// (css-color-5 §2).
///
/// Смешение идёт В ЗАЯВЛЕННОМ пространстве и заявленной дугой тона — тем же
/// `mix_in`, что считает точки градиента: смесь-стоп эталона и точка
/// градиента теста сходятся по построению.
pub(super) fn color_mix(body: &str) -> Option<(f32, f32, f32, f32)> {
    use crate::style::computed::GradSpace as S;
    let mut it = crate::style::css::split_args(body).into_iter();
    // css-color-5 §2.1: пара смешивается «as described in
    // [[css-color-4#interpolation]]», дугой тона управляет
    // <hue-interpolation-method>, по умолчанию shorter. Blink несёт
    // пространство и дугу до самого вычисления (`core/css/style_color.cc:301-303`,
    // `Color::FromColorMix(color_interpolation_space_, hue_interpolation_method_, …)`).
    // Прежде пространство выбрасывалось и смесь шла в гамма-sRGB:
    // `color-mix(in hsl longer hue, red, blue)` давал пурпур вместо лайма, и
    // эталон `gradient-longer-hue-{hsl,lch}-001-ref` рисовал ДРУГУЮ дугу, чем
    // тест, как только обе стороны ушли на растр (`gradient_as_tile`): 0.75/0.81.
    let head = it.next()?.trim().to_ascii_lowercase();
    let method = head.strip_prefix("in ").unwrap_or("");
    let space = match method.split_whitespace().next() {
        Some(
            "srgb-linear"
            | "xyz"
            | "xyz-d50"
            | "xyz-d65"
            | "display-p3-linear"
            | "rec2020-linear"
            | "a98-rgb-linear"
            | "prophoto-rgb-linear",
        ) => S::Linear,
        Some("oklab") => S::Oklab,
        Some("oklch") => S::Oklch,
        Some("lab") => S::Lab,
        Some("lch") => S::Lch,
        Some("hsl") => S::Hsl,
        Some("hwb") => S::Hwb,
        _ => S::Srgb,
    };
    // Дуга тона — коды `hue_arc`: 0 shorter, 1 longer, 2 increasing,
    // 3 decreasing (css-color-4 §12.4).
    let hue = if method.contains("longer") {
        1
    } else if method.contains("increasing") {
        2
    } else if method.contains("decreasing") {
        3
    } else {
        0
    };
    let one = |raw: &str| -> Option<((f32, f32, f32, f32), Option<f32>)> {
        let raw = raw.trim();
        // Доля стоит рядом с цветом и записывается процентом.
        let (color, share) = match raw.rfind('%') {
            Some(at) => {
                let head = raw[..at].trim_end();
                // `+1` резал бы многобайтный пробел (NBSP/U+3000) посреди
                // кода — шаг вперёд строго на ДЛИНУ найденного знака.
                let cut = head
                    .rfind(char::is_whitespace)
                    .map(|i| i + head[i..].chars().next().map_or(1, char::len_utf8))
                    .unwrap_or(0);
                let pct: f32 = head[cut..].parse().ok()?;
                (raw[..cut].trim(), Some(pct / 100.0))
            }
            None => (raw, None),
        };
        let c = interpolation_color(color)?;
        Some(((c.r, c.g, c.b, c.a), share))
    };
    let (first, p1) = one(it.next()?)?;
    let (second, p2) = one(it.next()?)?;
    let (w1, w2) = match (p1, p2) {
        (Some(a), Some(b)) if a + b > 0.0 => (a / (a + b), b / (a + b)),
        (Some(a), None) => (a, 1.0 - a),
        (None, Some(b)) => (1.0 - b, b),
        _ => (0.5, 0.5),
    };
    let alpha = first.3 * w1 + second.3 * w2;
    // Премультипликация (css-color-4 §12.3): для прямоугольных осей она
    // сводится к доле `w2·a2 / alpha` — тот же приём, что у `colour_at`
    // (background.rs). У непрозрачной пары доля остаётся `w2`, и `in srgb`
    // даёт прежнее `first·w1 + second·w2`.
    let k = if alpha > 0.0 {
        w2 * second.3 / alpha
    } else {
        w2
    };
    let colour = |c: (f32, f32, f32, f32)| crate::style::values::value::Color {
        r: c.0,
        g: c.1,
        b: c.2,
        a: c.3,
    };
    let (r, g, b) = mix_in(space, hue, colour(first), colour(second), k);
    Some((r, g, b, alpha))
}

/// Смешать два цвета в заданном пространстве интерполяции (css-color-4 §12).
///
/// Оси цвета переводятся в пространство, складываются с долей `k` и
/// переводятся обратно; полярные пространства ведут тон по выбранной дуге
/// (§12.4). Прозрачность сюда не входит — она линейна всегда и считается
/// вызывающим.
pub(super) fn mix_in(
    space: crate::style::computed::GradSpace,
    hue: u8,
    a: crate::style::values::value::Color,
    b: crate::style::values::value::Color,
    k: f32,
) -> (f32, f32, f32) {
    use crate::style::computed::GradSpace as S;
    let lerp = |x: f32, y: f32| x + (y - x) * k;
    match space {
        S::Srgb => gamut_map(lerp(a.r, b.r), lerp(a.g, b.g), lerp(a.b, b.b)),
        // Линейный свет: кривая sRGB снимается и возвращается. Все линейные
        // пространства дают тут один ответ (см. `GradSpace::Linear`).
        S::Linear => {
            let (ar, ag, ab) = (srgb_linear(a.r), srgb_linear(a.g), srgb_linear(a.b));
            let (br, bg, bb) = (srgb_linear(b.r), srgb_linear(b.g), srgb_linear(b.b));
            gamut_map(
                srgb_gamma(lerp(ar, br)),
                srgb_gamma(lerp(ag, bg)),
                srgb_gamma(lerp(ab, bb)),
            )
        }
        S::Oklab | S::Oklch => {
            let (al, aa, ab) = srgb_to_oklab(a);
            let (bl, ba, bb) = srgb_to_oklab(b);
            let (l, x, y) = if matches!(space, S::Oklch) {
                polar_mix(al, aa, ab, bl, ba, bb, hue, k)
            } else {
                (lerp(al, bl), lerp(aa, ba), lerp(ab, bb))
            };
            gamut_map_tuple(oklab_to_srgb(l, x, y))
        }
        S::Lab | S::Lch => {
            let (al, aa, ab) = srgb_to_lab(a);
            let (bl, ba, bb) = srgb_to_lab(b);
            let (l, x, y) = if matches!(space, S::Lch) {
                polar_mix(al, aa, ab, bl, ba, bb, hue, k)
            } else {
                (lerp(al, bl), lerp(aa, ba), lerp(ab, bb))
            };
            gamut_map_tuple(lab_to_srgb(l, x, y))
        }
        S::Hsl | S::Hwb => {
            let (ah, as_, al) = rgb_to_hsl(a);
            let (bh, bs, bl) = rgb_to_hsl(b);
            let h = hue_arc(ah, bh, hue, k);
            hsl_to_rgb(h, lerp(as_, bs), lerp(al, bl))
        }
    }
}

/// Смешение в ПОЛЯРНОЙ форме прямоугольного пространства: светлота и
/// цветность линейны, тон идёт по дуге (css-color-4 §12.4). Возврат — снова
/// прямоугольные оси, чтобы обратное преобразование было одно.
#[allow(clippy::too_many_arguments)]
fn polar_mix(
    al: f32,
    aa: f32,
    ab: f32,
    bl: f32,
    ba: f32,
    bb: f32,
    hue: u8,
    k: f32,
) -> (f32, f32, f32) {
    let (ac, ah) = ((aa * aa + ab * ab).sqrt(), ab.atan2(aa).to_degrees());
    let (bc, bh) = ((ba * ba + bb * bb).sqrt(), bb.atan2(ba).to_degrees());
    let l = al + (bl - al) * k;
    let c = ac + (bc - ac) * k;
    let h = hue_arc(ah, bh, hue, k).to_radians();
    (l, c * h.cos(), c * h.sin())
}

/// Тон на доле `k` по выбранной дуге (css-color-4 §12.4): 0 shorter,
/// 1 longer, 2 increasing, 3 decreasing. Углы приводятся к обороту, дуга
/// выбирается разностью, и только потом берётся доля — иначе `350°→10°`
/// поехало бы через весь круг.
fn hue_arc(from: f32, to: f32, method: u8, k: f32) -> f32 {
    let mut d = (to - from).rem_euclid(360.0);
    match method {
        1 => {
            if d > 180.0 {
                d -= 360.0;
            }
            if d > 0.0 {
                d -= 360.0;
            } else {
                d += 360.0;
            }
        }
        2 => {}
        3 => {
            if d > 0.0 {
                d -= 360.0;
            }
        }
        _ => {
            if d > 180.0 {
                d -= 360.0;
            }
        }
    }
    (from + d * k).rem_euclid(360.0)
}
