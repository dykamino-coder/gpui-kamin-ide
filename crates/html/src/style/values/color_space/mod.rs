//! Цветовые пространства CSS Color 4: `hwb()`, `lab()`, `lch()`, `oklab()`,
//! `oklch()`, `color()` и `color-mix()`.
//!
//! Зачем отдельным разбором. Все эти записи задают ОДИН И ТОТ ЖЕ цвет разными
//! системами координат, и привести их к точкам экрана можно только через
//! настоящее преобразование: сначала в XYZ, оттуда в линейный sRGB, оттуда —
//! степенной кривой в sRGB. Приблизить их подстановкой нельзя: `lab(50% 40 60)`
//! и близко не похож ни на один компонент записи.
//!
//! Числа матриц — из спецификации (CSS Color 4, приложение о преобразованиях).
//! Менять их «на глаз» нельзя: они согласованы между собой и с точками белого.

pub(crate) use gradient_stop::{
    colour_at as gradient_colour_at, out_of_gamut, parse as interpolation_color,
};

mod convert;
mod direct_gradient;
pub(crate) use direct_gradient::DirectGradient;
mod functions;
mod gradient_stop;
mod hsl_ramp;
pub(crate) use hsl_ramp::HslRamp;
mod icc;
mod matrices;
mod mix;
use convert::{
    gamut_map, gamut_map_tuple, hsl_to_rgb, lab_to_srgb, oklab_to_srgb, rgb_to_hsl, srgb_to_lab,
    srgb_to_oklab,
};
use functions::color_fn;
pub(crate) use functions::resolve_relative;
pub(crate) use icc::apply_icc;
pub use icc::load_profiles;
use icc::{PROFILES, icc_to_srgb};
use matrices::{
    A98_TO_XYZ, D50, D50_TO_D65, D65_TO_D50, LINEAR_SRGB_TO_XYZ, P3_TO_XYZ, PROPHOTO_TO_XYZ,
    REC2020_TO_XYZ, XYZ_TO_LINEAR_SRGB, mul, srgb_gamma, srgb_linear,
};
use mix::{color_mix, mix_in};

/// Разложить содержимое записи на числа и долю прозрачности.
///
/// Косая отделяет прозрачность, запятые и пробелы равноправны — так велит
/// современная запись (CSS Color 4 §4).
fn parts(inner: &str) -> (Vec<String>, f32) {
    let (body, alpha) = match inner.split_once('/') {
        Some((body, a)) => (body, number(a.trim(), 1.0).unwrap_or(1.0)),
        None => (inner, 1.0),
    };
    let list = body
        .split([',', ' ', '\t', '\n'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    (list, alpha.clamp(0.0, 1.0))
}

/// Число записи: доля от базы, `none` — ноль, обычное число — как есть.
fn number(raw: &str, base: f32) -> Option<f32> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("none") {
        return Some(0.0);
    }
    if let Some(pct) = raw.strip_suffix('%') {
        return pct.parse::<f32>().ok().map(|v| v / 100.0 * base);
    }
    raw.trim_end_matches("deg").parse().ok()
}

/// Разобрать одну из записей CSS Color 4. `None` — запись не наша.
pub fn parse(raw: &str) -> Option<(f32, f32, f32, f32)> {
    let text = raw.trim();
    let lower = text.to_ascii_lowercase();
    let inner = |name: &str| -> Option<&str> {
        lower
            .starts_with(&format!("{name}("))
            .then(|| text[name.len() + 1..text.len().saturating_sub(1)].trim())
    };
    if let Some(body) = inner("hwb") {
        return hwb(body);
    }
    // `contrast-color()` — чёрный или белый, что контрастнее к данному цвету
    // (css-color-5 §3): сравниваются отношения контраста к обоим.
    if let Some(body) = inner("contrast-color") {
        let c = crate::style::values::value::Color::parse(body)?;
        let y = 0.2126 * srgb_linear(c.r) + 0.7152 * srgb_linear(c.g) + 0.0722 * srgb_linear(c.b);
        let against_white = 1.05 / (y + 0.05);
        let against_black = (y + 0.05) / 0.05;
        return Some(if against_black >= against_white {
            (0.0, 0.0, 0.0, 1.0)
        } else {
            (1.0, 1.0, 1.0, 1.0)
        });
    }
    if let Some(body) = inner("lab") {
        return lab(body, false);
    }
    if let Some(body) = inner("oklab") {
        return lab(body, true);
    }
    if let Some(body) = inner("lch") {
        return lch(body, false);
    }
    if let Some(body) = inner("oklch") {
        return lch(body, true);
    }
    if let Some(body) = inner("color") {
        return color_fn(body);
    }
    if let Some(body) = inner("color-mix") {
        return color_mix(body);
    }
    None
}

/// `hwb(H W B[/A])` — тон, подмешанная белизна и чернота (§7).
fn hwb(body: &str) -> Option<(f32, f32, f32, f32)> {
    let (list, a) = parts(body);
    if list.len() < 3 {
        return None;
    }
    let h = number(&list[0], 1.0)? / 360.0;
    let w = number(&list[1], 1.0)?.clamp(0.0, 1.0);
    let b = number(&list[2], 1.0)?.clamp(0.0, 1.0);
    // Белизна с чернотой, вместе перекрывающие целое, дают ровно серый.
    if w + b >= 1.0 {
        let gray = w / (w + b);
        return Some((gray, gray, gray, a));
    }
    let rgba: gpui::Rgba = gpui::hsla(h, 1.0, 0.5, 1.0).into();
    let mix = |c: f32| c * (1.0 - w - b) + w;
    Some((mix(rgba.r), mix(rgba.g), mix(rgba.b), a))
}

/// `lab()`/`oklab()`: светлота и две оси цветности.
fn lab(body: &str, ok: bool) -> Option<(f32, f32, f32, f32)> {
    let (r, g, b, a) = gradient_stop::components(body, ok, false)?;
    let (r, g, b) = gamut_map(r, g, b);
    Some((r, g, b, a))
}

/// `lch()`/`oklch()`: та же светлота, но цветность задана длиной и углом.
fn lch(body: &str, ok: bool) -> Option<(f32, f32, f32, f32)> {
    let (r, g, b, a) = gradient_stop::components(body, ok, true)?;
    let (r, g, b) = gamut_map(r, g, b);
    Some((r, g, b, a))
}

#[cfg(test)]
mod tests {
    //! Тесты цветовых пространств: lab/lch/oklab, color(), color-mix(), относительный синтаксис.

    use super::*;

    /// Сравнение идёт по ЗАЖАТЫМ величинам: цвет шире sRGB выходит за
    /// пределы, и зажимает его разбор цвета, а не преобразование.
    fn close(a: (f32, f32, f32, f32), b: (f32, f32, f32), what: &str) {
        let a = (
            a.0.clamp(0.0, 1.0),
            a.1.clamp(0.0, 1.0),
            a.2.clamp(0.0, 1.0),
            a.3,
        );
        for (got, want) in [(a.0, b.0), (a.1, b.1), (a.2, b.2)] {
            assert!((got - want).abs() < 0.02, "{what}: {got} против {want}");
        }
    }

    /// Белый и красный обязаны совпасть во всех записях.
    #[test]
    fn known_colors_survive_the_conversion() {
        close(
            parse("lab(100% 0 0)").unwrap(),
            (1.0, 1.0, 1.0),
            "lab белый",
        );
        close(
            parse("oklch(0.628 0.2577 29.23)").unwrap(),
            (1.0, 0.0, 0.0),
            "oklch красный",
        );
        // Цвет шире охвата втягивается СЖАТИЕМ ЦВЕТНОСТИ (§13.1.5), а не
        // срезом: зелёный остаётся насыщенным зелёным, но уже не (0,1,0).
        let p3 = parse("color(display-p3 0 1 0)").unwrap();
        assert!(
            p3.1 > 0.9 && p3.0 < 0.3 && p3.2 < 0.5,
            "p3 зелёный после втягивания: {p3:?}"
        );
        close(
            parse("color(srgb 0.2 0.4 0.6)").unwrap(),
            (0.2, 0.4, 0.6),
            "srgb как есть",
        );
    }

    /// `hwb`: белизна с чернотой в сумме за единицу дают серый.
    #[test]
    fn hwb_mixes_white_and_black() {
        close(parse("hwb(0 100% 0%)").unwrap(), (1.0, 1.0, 1.0), "белый");
        close(parse("hwb(0 0% 100%)").unwrap(), (0.0, 0.0, 0.0), "чёрный");
        close(parse("hwb(0 50% 50%)").unwrap(), (0.5, 0.5, 0.5), "серый");
    }

    /// Доли смешивания приводятся к единице.
    #[test]
    fn color_mix_honours_shares() {
        close(
            parse("color-mix(in srgb, white 25%, black 75%)").unwrap(),
            (0.25, 0.25, 0.25),
            "четверть белого",
        );
    }
}
