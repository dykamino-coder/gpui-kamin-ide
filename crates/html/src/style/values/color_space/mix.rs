//! color-mix(): смешение в прямоугольных и полярных пространствах, интерполяция оттенка.

use super::*;
use crate::document::state::ValueCx;

/// `color-mix(in <пространство> [<дуга> hue]?, <цвет> <доля>?, <цвет> <доля>?)`
/// (css-color-5 §2).
///
/// Смешение идёт В ЗАЯВЛЕННОМ пространстве и заявленной дугой тона — тем же
/// `mix_in`, что считает точки градиента: смесь-стоп эталона и точка
/// градиента теста сходятся по построению.
pub(super) fn color_mix(body: &str, vcx: ValueCx<'_>) -> Option<(f32, f32, f32, f32)> {
    color_mix_impl(body, true, vcx)
}

pub(super) fn color_mix_unmapped(body: &str, vcx: ValueCx<'_>) -> Option<(f32, f32, f32, f32)> {
    color_mix_impl(body, false, vcx)
}

fn color_mix_impl(body: &str, mapped: bool, vcx: ValueCx<'_>) -> Option<(f32, f32, f32, f32)> {
    super::style_color::absolute_mix(body, mapped, vcx)
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
    mix_in_impl(space, hue, a, b, k, true)
}

pub(super) fn mix_in_impl(
    space: crate::style::computed::GradSpace,
    hue: u8,
    a: crate::style::values::value::Color,
    b: crate::style::values::value::Color,
    k: f32,
    mapped: bool,
) -> (f32, f32, f32) {
    use crate::style::computed::GradSpace as S;
    let map = |r, g, b| {
        if mapped {
            gamut_map(r, g, b)
        } else {
            (r, g, b)
        }
    };
    let map_tuple = |v| if mapped { gamut_map_tuple(v) } else { v };
    let lerp = |x: f32, y: f32| x + (y - x) * k;
    match space {
        S::Srgb => map(lerp(a.r, b.r), lerp(a.g, b.g), lerp(a.b, b.b)),
        // Линейный свет: кривая sRGB снимается и возвращается. Все линейные
        // пространства дают тут один ответ (см. `GradSpace::Linear`).
        S::Linear => {
            let (ar, ag, ab) = (srgb_linear(a.r), srgb_linear(a.g), srgb_linear(a.b));
            let (br, bg, bb) = (srgb_linear(b.r), srgb_linear(b.g), srgb_linear(b.b));
            map(
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
            map_tuple(oklab_to_srgb(l, x, y))
        }
        S::Lab | S::Lch => {
            let (al, aa, ab) = srgb_to_lab(a);
            let (bl, ba, bb) = srgb_to_lab(b);
            let (l, x, y) = if matches!(space, S::Lch) {
                polar_mix(al, aa, ab, bl, ba, bb, hue, k)
            } else {
                (lerp(al, bl), lerp(aa, ba), lerp(ab, bb))
            };
            map_tuple(lab_to_srgb(l, x, y))
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
pub(super) fn hue_arc(from: f32, to: f32, method: u8, k: f32) -> f32 {
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
