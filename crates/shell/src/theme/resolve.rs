//! Разрешение палитры из contributed-темы: ключи, hex, кандидаты акцента.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::theme::color_math::bg_surfaces;
use crate::theme::color_math::chroma_of;
use crate::theme::color_math::close;
use crate::theme::color_math::lightness;
use crate::theme::color_math::mixc;
use crate::theme::color_math::saturation;
use crate::theme_sync::DISABLED_T;
use crate::theme_sync::MUTED_T;
use crate::theme_sync::NEUTRAL_MAX_CHROMA;
use kamin_theme::Color;

/// Нейтральные surface-ключи темы — ranked by lightness строят ramp.
const NEUTRAL_SURFACE_KEYS: [&str; 12] = [
    "editor.background",
    "sideBar.background",
    "editorWidget.background",
    "dropdown.background",
    "activityBar.background",
    "titleBar.activeBackground",
    "panel.background",
    "editorGroupHeader.tabsBackground",
    "sideBarSectionHeader.background",
    "list.activeSelectionBackground",
    "input.background",
    "statusBar.background",
];
/// Accent = самый НАСЫЩЕННЫЙ авторский кандидат (Dracula pink, не grey-blue).
const ACCENT_CANDIDATES: [&str; 9] = [
    "activityBarBadge.background",
    "progressBar.background",
    "button.background",
    "focusBorder",
    "textLink.foreground",
    "panelTitle.activeBorder",
    "tab.activeBorder",
    "panel.border",
    "list.activeSelectionForeground",
];
/// Чистый резолв (тестируемый): colors-JSON + база → палитра.
pub(crate) fn resolve_palette(
    colors: &serde_json::Value,
    dark: bool,
    base: kamin_theme::Palette,
) -> kamin_theme::Palette {
    let mut p = base;
    let raw = |k: &str| colors.get(k).and_then(|c| c.as_str());
    let get = |keys: &[&str]| -> Option<Color> {
        keys.iter().find_map(|k| raw(k).and_then(parse_theme_hex))
    };

    // ── Текст + вторичные акценты (first-wins, KAMIN_TOKEN_MAP)
    if let Some(c) = get(&["foreground", "editor.foreground"]) {
        p.text_primary = c;
        p.text_secondary = c;
    }
    if let Some(c) = get(&["descriptionForeground", "foreground"]) {
        p.text_subtext = c;
    }
    if let Some(c) = get(&["textLink.foreground", "terminal.ansiBlue"]) {
        p.accent_blue = c;
    }
    if let Some(c) = get(&[
        "errorForeground",
        "editorError.foreground",
        "terminal.ansiRed",
    ]) {
        p.accent_red = c;
    }
    if let Some(c) = get(&[
        "terminal.ansiGreen",
        "gitDecoration.addedResourceForeground",
    ]) {
        p.accent_green = c;
    }
    if let Some(c) = get(&["editorWarning.foreground", "terminal.ansiYellow"]) {
        p.accent_yellow = c;
    }

    // ── Редактор/терминал
    let editor_bg = get(&["editor.background"]);
    let editor_fg = get(&["editor.foreground", "foreground"]);
    if let Some(c) = editor_bg {
        p.editor_bg = c;
    }
    if let Some(c) = editor_fg {
        p.editor_fg = c;
        p.editor_cursor = get(&["editorCursor.foreground"]).unwrap_or(c);
    }

    // ── Elevation-ramp: нейтральные ОПАКОВЫЕ авторские поверхности
    let mut stops: Vec<Color> = Vec::new();
    for k in NEUTRAL_SURFACE_KEYS {
        if let Some(s) = raw(k)
            && theme_hex_opaque(s)
            && let Some(c) = parse_theme_hex(s)
            && chroma_of(c) <= NEUTRAL_MAX_CHROMA
            && !stops.iter().any(|e| close(*e, c))
        {
            stops.push(c);
        }
    }
    stops.sort_by(|a, b| lightness(*a).total_cmp(&lightness(*b)));
    let s = bg_surfaces(&stops, dark, editor_bg);
    let bg_fallback = editor_bg.or_else(|| get(&["sideBar.background"]));
    if let Some(c) = s.panel {
        p.bg_base = c;
        p.bg_primary = c;
        p.bg_mantle = c;
    }
    if let Some(c) = s.backdrop.or(bg_fallback) {
        p.bg_sidebar = c;
    }
    if let Some(c) = s.surface {
        p.bg_surface = c;
    }
    if let Some(c) = s.surface_hover {
        p.bg_surface_hover = c;
    }
    if let Some(c) = s.overlay {
        p.bg_overlay = c;
        p.bg_overlay_hover = c;
    }
    // Glint-рамка карт: середина градиента = bg_base ТЕМЫ (оригинал:
    // `linear-gradient(135deg, edge 0%, var(--bg-base) 22%…78%, edge 100%)`;
    // light-вариант .mainPanel берёт bg_surface) — без переклада contributed-
    // темы держали Kamin-цвет #262533, и на почти чёрных темах (CBD) рамка
    // не «уходила в фон» на верх-право/низ-лево (скрин юзера). Края
    // (белый/тёплый 18%) — от base-палитры.
    p.glint_mid = if dark { p.bg_base } else { p.bg_surface };

    // ── Accent = самый насыщенный кандидат
    let accent = ACCENT_CANDIDATES
        .iter()
        .filter_map(|k| raw(k).and_then(parse_theme_hex))
        .max_by(|a, b| saturation(*a).total_cmp(&saturation(*b)));
    if let Some(c) = accent {
        p.accent_primary = c;
        p.accent_action = c;
    }

    // ── Производные ТОЛЬКО при отсутствии: muted/disabled к фону
    let fg = get(&["foreground", "editor.foreground"]);
    let bg = get(&["editor.background", "sideBar.background"]);
    let muted =
        get(&["descriptionForeground"]).or_else(|| fg.zip(bg).map(|(f, b)| mixc(f, b, MUTED_T)));
    if let Some(c) = muted {
        p.text_muted = c;
        p.text_subtext = c;
    }
    let disabled =
        get(&["disabledForeground"]).or_else(|| fg.zip(bg).map(|(f, b)| mixc(f, b, DISABLED_T)));
    if let Some(c) = disabled {
        p.text_disabled = c;
    }
    p
}
/// "#rgb/#rrggbb/#rrggbbaa" → Color (альфа отбрасывается — палитра без альф).
pub(crate) fn parse_theme_hex(s: &str) -> Option<Color> {
    let hex = s.strip_prefix('#')?;
    let full = match hex.len() {
        3 | 4 => hex[..3].chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => hex.to_string(),
        8 => hex[..6].to_string(),
        _ => return None,
    };
    u32::from_str_radix(&full, 16).ok().map(Color::hex)
}
/// `#rrggbbaa` с альфой < ff — полупрозрачный ТИНТ, не сплошная поверхность.
pub(crate) fn theme_hex_opaque(s: &str) -> bool {
    let Some(hex) = s.strip_prefix('#') else {
        return true;
    };
    match hex.len() {
        8 => hex[6..].eq_ignore_ascii_case("ff"),
        4 => hex[3..].eq_ignore_ascii_case("f"),
        _ => true,
    }
}
