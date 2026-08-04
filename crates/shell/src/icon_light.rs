//! `TreeIcon.module.css:6` — `[data-theme="light"] .img { filter: saturate(3.2)
//! brightness(0.7) }`. CSS-фильтров в gpui нет — та же цветовая матрица
//! применяется К ПИКСЕЛЯМ уже растеризованной иконки (`icon_raster`).
//! Переписывание hex внутри SVG-исходника ломало `url(#id)`-ссылки и дефы
//! (битые иконки светлой темы) — этот путь удалён.

/// `saturate(3.2)` — первый фильтр цепочки.
const SATURATE: f32 = 3.2;
/// `brightness(0.7)` — второй, применяется к результату первого.
const BRIGHTNESS: f32 = 0.7;

// Коэффициенты яркости из Filter Effects (feColorMatrix type="saturate").
const LR: f32 = 0.213;
const LG: f32 = 0.715;
const LB: f32 = 0.072;

/// Один пиксель фильтра: saturate → brightness, в sRGB (shorthand-функции CSS
/// работают именно в нём, в отличие от SVG-фильтров с linearRGB). Матрица
/// линейна, поэтому применима и к premultiplied-значениям (альфа не трогается).
pub(crate) fn filter_rgb(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let (r, g, b) = (
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    );
    let s = SATURATE;
    let nr = (LR + 0.787 * s) * r + (LG - LG * s) * g + (LB - LB * s) * b;
    let ng = (LR - LR * s) * r + (LG + 0.285 * s) * g + (LB - LB * s) * b;
    let nb = (LR - LR * s) * r + (LG - LG * s) * g + (LB + 0.928 * s) * b;
    let q = |v: f32| ((v.clamp(0.0, 1.0) * BRIGHTNESS).clamp(0.0, 1.0) * 255.0).round() as u8;
    (q(nr), q(ng), q(nb))
}

#[cfg(test)]
mod tests {
    use super::filter_rgb;

    #[test]
    fn saturate_then_brightness() {
        // Серый насыщать нечего — остаётся только brightness 0.7
        assert_eq!(filter_rgb(100, 100, 100), (70, 70, 70));
        // Канал уходит в клиппинг, дальше только brightness: 255·0.7 = 178.5
        assert_eq!(filter_rgb(255, 0, 0).0, 179);
        assert_eq!(filter_rgb(0, 0, 0), (0, 0, 0));
    }
}
