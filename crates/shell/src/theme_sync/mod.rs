//! Синхронизация темы gpui-component с палитрой KaminIDE: Theme::change
//! (полный dark/light сет, включая highlight-тему редактора) + наши
//! оверрайды (скроллбары, инпуты, сплиттеры). Вызывается на старте и при
//! смене темы (Appearance-поповер). Здесь — константы resolve-порта, кэш
//! темы и вход contributed-темы; переклад компонентов в `components`,
//! tokenColors → подсветка в `syntax`.
//!
//! Contributed-маппер — ПОЛНЫЙ порт contributed-theme-resolve.ts:
//! elevation-ramp из авторских нейтральных поверхностей (backdrop → panel →
//! card → overlay, якорь на editor.background, light-темы инвертируют),
//! accent = самый НАСЫЩЕННЫЙ кандидат, muted/disabled — blend при отсутствии.

mod components;
mod syntax;
#[cfg(test)]
mod tests;

pub use components::apply;
pub use syntax::set_contrib_syntax;

use crate::theme::resolve::resolve_palette;
use gpui::App;
use kamin_theme::{Color, ThemeKind};
use syntax::token_colors_to_syntax;

// Блендинг отсутствующих ключей (доли пути fg→bg) — как в оригинале
pub(crate) const MUTED_T: f32 = 0.42;
pub(crate) const DISABLED_T: f32 = 0.62;
pub(crate) const NEUTRAL_MAX_CHROMA: f32 = 0.25;
pub(crate) const MID_L: f32 = 0.5;
pub(crate) const MIN_SEP: f32 = 0.03;
pub(crate) const BACKDROP_NUDGE: f32 = 0.4;
pub(crate) const PANEL_NUDGE: f32 = 0.12;
pub(crate) const SURFACE_MAX_STEP: f32 = 0.09;
pub(crate) const OVERLAY_MAX_STEP: f32 = 0.13;

pub(crate) const BLACK: Color = Color::hex(0x000000);
pub(crate) const WHITE: Color = Color::hex(0xffffff);

/// Файл-кэш последней применённой contributed-темы: применяется СИНХРОННО на
/// старте, до загрузки расширения-поставщика — иначе бут шёл на дефолтной
/// теме до прихода реестра (жалоба юзера).
pub fn theme_cache_path() -> std::path::PathBuf {
    let (_, cache) = crate::host_link::data_dirs();
    cache.join("theme-cache.json")
}

/// Применить закэшированную contributed-тему на буте. true — применена;
/// false — кэша нет/бит (бут остаётся на builtin-выборе).
pub fn apply_cached_contributed(cx: &mut App) -> bool {
    let id = crate::layout_store::load_raw_key("contributedThemeId")
        .and_then(|v| v.as_str().map(str::to_string));
    if id.is_none() {
        return false;
    }
    let dark_ui = crate::layout_store::load_raw_key("contributedThemeDarkUi")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let path = theme_cache_path();
    if !path.exists() {
        return false;
    }
    apply_contributed(&path.to_string_lossy(), dark_ui, cx)
}

/// Contributed VS Code-тема: JSON colors → Palette через полный resolve-порт.
/// Возвращает false если файл не прочитался.
pub fn apply_contributed(path: &str, dark_ui: bool, cx: &mut App) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    // Кэш для мгновенного применения на следующем буте (см. выше). Пишем
    // ИСХОДНЫЙ текст темы: применение на буте пройдёт тот же resolve-путь.
    {
        let cache = theme_cache_path();
        if cache.to_string_lossy() != path {
            if let Some(dir) = cache.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&cache, &text);
        }
    }
    // Тем-JSON бывает с комментариями (jsonc) — срежем // построчно
    let clean: String = text
        .lines()
        .map(|l| {
            if let Some(i) = l.find("//")
                && (!l[..i].contains('"') || l[..i].matches('"').count() % 2 == 0)
            {
                return &l[..i];
            }
            l
        })
        .collect::<Vec<_>>()
        .join("\n");
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&clean) else {
        return false;
    };
    let colors = v.get("colors").cloned().unwrap_or_default();
    let base = if dark_ui {
        ThemeKind::Dark
    } else {
        ThemeKind::Light
    };
    let p = resolve_palette(&colors, dark_ui, *base.palette_base());
    kamin_theme::set_contributed(Some(p));
    // Вебвью получают СЫРУЮ colors-семью темы поверх дефолтов (как оригинал
    // пробрасывает --vscode-* в webview) — рамп-палитра тут не годится:
    // чат вязался на backdrop вместо editor.background (скрин юзера).
    crate::ui::webview_theme::set_contrib_colors(colors.as_object().map(|m| {
        m.iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect()
    }));
    // tokenColors → подсветка gpui-редактора (#71); apply() ниже вольёт
    // оверлей в highlight-тему.
    set_contrib_syntax(token_colors_to_syntax(&v));
    apply(base, cx);
    true
}
