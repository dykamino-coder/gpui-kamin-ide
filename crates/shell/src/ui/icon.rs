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

/// Спиннер загрузки (`codicon-loading` оригинала): глиф-спиннер с общего
/// такта (`glyph_spinner`) вместо вращающейся иконки — вращение на редких
/// кадрах дёргалось, а на частых перерисовывало всё окно (INC-2026-0001).
pub fn spinner(
    _id: impl Into<gpui::ElementId>,
    size_px: f32,
    color: gpui::Rgba,
) -> gpui::AnyElement {
    super::glyph_spinner::glyph_spinner(size_px, color).into_any_element()
}

/// `.spinner` вебвью-лоадера: тот же глиф-спиннер цветом дуги (кольцо
/// оригинала не рисуется — вращения больше нет).
pub fn spinner_ring(
    _id: impl Into<gpui::ElementId>,
    size_px: f32,
    _ring: gpui::Rgba,
    arc: gpui::Rgba,
) -> Div {
    super::glyph_spinner::glyph_spinner(size_px, arc)
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
