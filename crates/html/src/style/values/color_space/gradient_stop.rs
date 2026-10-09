//! Retain extended RGB coordinates for CIE/OK gradient stop interpolation.

use crate::style::values::value::Color;

pub(super) fn components(body: &str, ok: bool, polar: bool) -> Option<(f32, f32, f32, f32)> {
    let (list, alpha) = super::parts(body);
    if list.len() < 3 {
        return None;
    }
    let lmax = if ok { 1.0 } else { 100.0 };
    let lightness = super::number(&list[0], lmax)?.clamp(0.0, lmax);
    let (x, y) = if polar {
        let chroma = super::number(&list[1], if ok { 0.4 } else { 150.0 })?;
        let hue = super::number(&list[2], 1.0)?.to_radians();
        (chroma * hue.cos(), chroma * hue.sin())
    } else {
        let base = if ok { 0.4 } else { 125.0 };
        (
            super::number(&list[1], base)?,
            super::number(&list[2], base)?,
        )
    };
    let (r, g, b) = if ok {
        super::oklab_to_srgb(lightness, x, y)
    } else {
        super::lab_to_srgb(lightness, x, y)
    };
    Some((r, g, b, alpha))
}

pub(crate) fn parse(raw: &str) -> Option<Color> {
    let text = raw.trim();
    let lower = text.to_ascii_lowercase();
    for (name, ok, polar) in [
        ("lab", false, false),
        ("lch", false, true),
        ("oklab", true, false),
        ("oklch", true, true),
    ] {
        if lower.starts_with(&format!("{name}(")) && text.ends_with(')') {
            // CSS Color 4 §Color Interpolation: gamut mapping follows
            // interpolation. Mapping each stop first changes its CIE/OK
            // coordinates, even when the interpolated color is in gamut.
            let (r, g, b, a) = components(&text[name.len() + 1..text.len() - 1], ok, polar)?;
            return Some(Color { r, g, b, a });
        }
    }
    Color::parse(text)
}

fn display(color: Color) -> Color {
    let (r, g, b) = super::gamut_map(color.r, color.g, color.b);
    Color {
        r,
        g,
        b,
        a: color.a,
    }
}

pub(crate) fn out_of_gamut(color: Color) -> bool {
    [color.r, color.g, color.b]
        .into_iter()
        .any(|c| !(0.0..=1.0).contains(&c))
}

pub(crate) fn colour_at(
    stops: &[(crate::style::values::value::Color, f32)],
    t: f32,
    space: crate::style::computed::GradSpace,
    hue: u8,
) -> crate::style::values::value::Color {
    let Some(first) = stops.first() else {
        return crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
    };
    if t <= first.1 {
        return display(first.0);
    }
    for pair in stops.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if t >= a.1 && t <= b.1 {
            let k = if b.1 > a.1 {
                (t - a.1) / (b.1 - a.1)
            } else {
                1.0
            };
            // Цвета смешиваются УЖЕ в пространстве интерполяции
            // (css-color-4 §12.2): перевод туда, покомпонентная доля,
            // перевод обратно. Прозрачность живёт отдельно от осей цвета
            // и всегда линейна.
            // Премультипликация (css-images-3 §3.5.3, css-color-4 §12.3):
            // для прямоугольных осей она равна доле `k·a1 / alpha`.
            let alpha = a.0.a + (b.0.a - a.0.a) * k;
            let kc = if alpha > 0.0 { k * b.0.a / alpha } else { k };
            let (r, g, bl) = crate::style::values::color_space::mix_in(space, hue, a.0, b.0, kc);
            return crate::style::values::value::Color {
                r,
                g,
                b: bl,
                a: alpha,
            };
        }
    }
    display(stops.last().map(|s| s.0).unwrap_or(first.0))
}
