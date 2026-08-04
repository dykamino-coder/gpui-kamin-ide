//! Codicon-глифы (@vscode/codicons, тот же ttf, что шипит kamin-ide).
//! Мапа id→глиф пополняется по мере надобности (contributions — фаза exthost).

use gpui::prelude::*;
use gpui::{Div, div, px};

pub const CHROME_CLOSE: &str = "\u{eab8}";
pub const CHROME_MAXIMIZE: &str = "\u{eab9}";
pub const CHROME_MINIMIZE: &str = "\u{eaba}";
pub const CHROME_RESTORE: &str = "\u{eabb}";
pub const CHEVRON_RIGHT: &str = "\u{eab6}";
pub const CHEVRON_DOWN: &str = "\u{eab4}";
pub const ADD: &str = "\u{ea60}";
// delete — подключается с ConfirmModal (деструктив требует подтверждения)
pub const SEARCH: &str = "\u{ea6d}";
pub const FA_CIRCLE_PLUS: &str = "\u{f055}";

// FontAwesome 7 Free (тот же woff2, что шипит kamin-ide);
// solid = weight 900 внутри WWS-семейства DirectWrite
pub const FA_FAMILY: &str = "Font Awesome 7 Free";
pub const FA_GEAR: &str = "\u{f013}";
pub const FA_TABLE_COLUMNS: &str = "\u{f0db}";
pub const FA_BUG: &str = "\u{f188}";
pub const FA_PLUS: &str = "\u{2b}";

/// FontAwesome-глиф (solid, weight 900) в боксе 16×16.
pub fn fa(glyph: &'static str, font_px: f32) -> Div {
    div()
        .font_family(FA_FAMILY)
        .font_weight(gpui::FontWeight::BLACK)
        // FA7: `.fa { width: var(--fa-width, 1.25em); line-height: 1 }` —
        // бокс пропорционален кеглю; жёсткие 16×16 раздували пилюли с
        // мелкими глифами (ревью ц.15)
        .w(px(font_px * 1.25))
        .h(px(font_px))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(font_px))
        .child(glyph)
}

/// Кодикон с динамическим глифом (из `codicon_map`): та же геометрия, что у
/// `codicon()`, но глиф не обязан быть `'static`.
pub fn codicon_str(glyph: &str, font_px: f32) -> Div {
    div()
        .font_family("codicon")
        .w(px(font_px))
        .h(px(font_px))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(font_px))
        .child(glyph.to_string())
}

/// Кодикон. Бокс = кегль: в браузере `<i class="codicon">` — инлайн, его
/// ширина равна advance глифа, а у codicon-шрифта advance ровно 1em. Жёсткие
/// 16×16 добавляли лишние 4px к слоту при шрифте 12 (замер ц.8: пилюля search
/// 163.2 против 157.6, зазор иконка→текст 19.2 против 16.8).
pub fn codicon(glyph: &'static str, font_px: f32) -> Div {
    div()
        .font_family("codicon")
        .w(px(font_px))
        .h(px(font_px))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(font_px))
        .child(glyph)
}

/// `codicon-loading` + `codicon-modifier-spin`: тот же глиф, но SVG-ассетом —
/// текст в gpui не поворачивается, а `svg()` умеет `with_transformation`.
/// Кадры дискретные, как у оригинала: `animation: codicon-spin 1.5s steps(30)`
/// (`codicon.css:35-40`), то есть 30 положений по 12°.
pub fn spinner(
    id: impl Into<gpui::ElementId>,
    size_px: f32,
    color: gpui::Rgba,
) -> gpui::AnimationElement<gpui::Svg> {
    use gpui::{Animation, AnimationExt as _, Transformation, percentage};
    // `codicon.css:35-39` задаёт `codicon-spin 1.5s steps(30) infinite`, но
    // `:50-53` перебивает ИМЕННО для `.codicon-loading`:
    // `animation-duration: 1s !important` +
    // `animation-timing-function: cubic-bezier(.53,.21,.29,.67) !important`.
    // `!important` бьёт шорткат, значит шагов НЕТ и период 1000 мс —
    // мы крутили 1500 мс дискретно по 30 шагов (ревью ц.25).
    const PERIOD_MS: u64 = 1000;
    gpui::svg()
        .path("icons/codicon-loading.svg")
        .w(px(size_px))
        .h(px(size_px))
        .flex_shrink_0()
        .text_color(color)
        .with_animation(
            id,
            Animation::new(std::time::Duration::from_millis(PERIOD_MS))
                .repeat()
                .with_easing(cubic_bezier_53_21_29_67),
            |el, delta| el.with_transformation(Transformation::rotate(percentage(delta))),
        )
}

/// `cubic-bezier(0.53, 0.21, 0.29, 0.67)` спиннера кодиконов: по x решаем
/// уравнение Безье методом бисекции (аналитического обратного нет), по y —
/// подставляем найденный параметр. 12 итераций дают точность ~2e-4.
fn cubic_bezier_53_21_29_67(x: f32) -> f32 {
    const X1: f32 = 0.53;
    const Y1: f32 = 0.21;
    const X2: f32 = 0.29;
    const Y2: f32 = 0.67;
    fn bezier(t: f32, p1: f32, p2: f32) -> f32 {
        let u = 1.0 - t;
        3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
    }
    let x = x.clamp(0.0, 1.0);
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..12 {
        let mid = 0.5 * (lo + hi);
        if bezier(mid, X1, X2) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bezier(0.5 * (lo + hi), Y1, Y2)
}

/// `.spinner` вебвью-лоадера: кольцо `border: 2.5px` цвета text-primary 16 %
/// с верхней четвертью accent-action, вращение `.7s linear infinite`
/// (`WebviewPanelView.module.css:33-40`). Кольцо статично, крутится только
/// дуга — в gpui поворачивается лишь `svg()`.
pub fn spinner_ring(
    id: impl Into<gpui::ElementId>,
    size_px: f32,
    ring: gpui::Rgba,
    arc: gpui::Rgba,
) -> Div {
    use gpui::{Animation, AnimationExt as _, Transformation, percentage};
    const PERIOD_MS: u64 = 700;
    const BORDER: f32 = 2.5;
    div()
        .relative()
        .w(px(size_px))
        .h(px(size_px))
        .flex_shrink_0()
        .rounded_full()
        .border(px(BORDER))
        .border_color(ring)
        .child(
            gpui::svg()
                .path("icons/spinner-arc.svg")
                .absolute()
                // Дуга рисуется по тому же радиусу, что и рамка: бокс кольца
                // без границ, сдвинутый на её толщину
                .top(px(-BORDER))
                .left(px(-BORDER))
                .w(px(size_px))
                .h(px(size_px))
                .text_color(arc)
                .with_animation(
                    id,
                    Animation::new(std::time::Duration::from_millis(PERIOD_MS)).repeat(),
                    |el, delta| el.with_transformation(Transformation::rotate(percentage(delta))),
                ),
        )
}

/// `data:image/<fmt>;base64,<...>` → картинка для `gpui::img`.
/// Ветка `isImageIcon` оригинала (`signals/activity.ts:89`) принимает и
/// `data:`; движок сам такие URI не декодирует.
pub fn data_uri_image(uri: &str) -> Option<std::sync::Arc<gpui::Image>> {
    use base64::Engine as _;
    let rest = uri.strip_prefix("data:")?;
    let (meta, payload) = rest.split_once(',')?;
    if !meta.contains("base64") {
        return None;
    }
    let format = match meta.split(';').next()?.trim() {
        "image/png" => gpui::ImageFormat::Png,
        "image/jpeg" | "image/jpg" => gpui::ImageFormat::Jpeg,
        "image/webp" => gpui::ImageFormat::Webp,
        "image/gif" => gpui::ImageFormat::Gif,
        "image/bmp" => gpui::ImageFormat::Bmp,
        "image/svg+xml" => gpui::ImageFormat::Svg,
        _ => return None,
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .ok()?;
    Some(std::sync::Arc::new(gpui::Image::from_bytes(format, bytes)))
}
