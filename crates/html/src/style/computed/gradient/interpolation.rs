//! Авторский синтаксис цвета выбирает одно пространство интерполяции и одну дугу оттенка.

use super::GradSpace;

pub(crate) fn interpolation_method(interp: Option<&str>, all_legacy: bool) -> (GradSpace, u8) {
    // Дуга тона нужна ЛЮБОМУ полярному пространству, не только `hsl`
    // (css-color-4 §12.4); умолчание — shorter.
    let hue_arc: u8 = interp.map_or(0, |i| {
        if i.contains("longer") {
            1
        } else if i.contains("increasing") {
            2
        } else if i.contains("decreasing") {
            3
        } else {
            0
        }
    });
    let named_space: Option<GradSpace> =
        interp
            .and_then(|i| i.split_whitespace().next())
            .and_then(|s| match s {
                "srgb" => Some(GradSpace::Srgb),
                "srgb-linear"
                | "xyz"
                | "xyz-d50"
                | "xyz-d65"
                | "display-p3-linear"
                | "rec2020-linear"
                | "a98-rgb-linear"
                | "prophoto-rgb-linear" => Some(GradSpace::Linear),
                "oklab" => Some(GradSpace::Oklab),
                "oklch" => Some(GradSpace::Oklch),
                "lab" => Some(GradSpace::Lab),
                "lch" => Some(GradSpace::Lch),
                "hsl" => Some(GradSpace::Hsl),
                "hwb" => Some(GradSpace::Hwb),
                // Other encoded RGB spaces retain the legacy sRGB route.
                _ => None,
            });
    (
        named_space.unwrap_or(if all_legacy {
            GradSpace::Srgb
        } else {
            GradSpace::Oklab
        }),
        hue_arc,
    )
}

/// Записан ли цвет УСТАРЕВШЕЙ формой sRGB: имя, `#hex`, `rgb()`, `rgba()`,
/// `hsl()`, `hsla()`, `hwb()` и их формы с прозрачностью (css-color-4 §12.2).
/// От ответа зависит пространство интерполяции по умолчанию.
pub(crate) fn legacy_srgb_color(token: &str) -> bool {
    let t = token.trim().to_ascii_lowercase();
    if t.starts_with('#') {
        return true;
    }
    match t.split_once('(') {
        // `rgb()` с `none` устаревшей записью не выражается: Blink вычисляет
        // его в `color(srgb …)`, и умолчание смешения становится OKLab
        // (`gradient-analogous-missing-components-004`, `gradient-eval-004`).
        // У `hsl()`/`hwb()` пространство и с `none` остаётся устаревшим.
        Some((name, body)) => match name.trim() {
            "rgb" | "rgba" => !body.contains("none"),
            "hsl" | "hsla" | "hwb" => true,
            _ => false,
        },
        // Имя цвета, `transparent` и `currentcolor` — тоже устаревшие формы.
        None => true,
    }
}
