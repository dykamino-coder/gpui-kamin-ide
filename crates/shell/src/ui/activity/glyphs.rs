//! Глифы активности: phosphor/codicon и подписи.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use gpui::prelude::*;
use gpui::{AnyElement, SharedString, px, svg};

/// Иконки built-in активностей (activity.ts BUILTIN_ACTIVITIES):
/// Phosphor SVG для известных токенов, None = фолбэк codicon-шрифт.
pub fn phosphor_path(icon: &str) -> Option<&'static str> {
    Some(match icon {
        "projects" | "folders" => "icons/folders.svg",
        "tree" | "tree-view" => "icons/tree-view.svg",
        "search" => "icons/search.svg",
        "problems" | "warning" => "icons/warning.svg",
        "terminal" => "icons/terminal.svg",
        "customize" | "gear" => "icons/gear.svg",
        _ => return None,
    })
}
/// codicon-фолбэк (VSIX-иконки без Phosphor-глифа; Bridge-вклады —
/// плейсхолдеры до чтения registry)
pub fn codicon_glyph(icon: &str) -> &'static str {
    match icon {
        // Bridge-вклады шлют id контейнера, а не имя кодикона
        "claudeBridgePlan" => "\u{eab3}",       // checklist
        "claudeBridgeTodos" => "\u{eb67}",      // tasklist
        "claudeBridgeAgents" => "\u{ec20}",     // robot
        "claudeBridgeToolsUsage" => "\u{eb1c}", // tools
        // Остальное — ОБЩАЯ карта имён: своя 4-строчная таблица её
        // игнорировала и отдавала codicon-file даже для известных имён
        // (например `circle-large`) — ревью ц.12
        other => crate::ui::codicon_map::codicon_by_name(other).unwrap_or("\u{ea7b}"),
    }
}
/// Человекочитаемый label активности (BUILTIN_ACTIVITIES + Bridge-вклады)
/// для тултипа.
/// Лейбл для тултипа: сперва ОБЩИЙ реестр (`ActivityBar.tsx:88`
/// `data-tooltip={item.label}` — там и contributed-тулы), затем встроенная
/// таблица как фолбэк (ревью ц.21: у contributed-тулов тултипа не было).
pub fn activity_tooltip(id: &str) -> String {
    if let Some((label, _)) = crate::activity::lookup_any(id)
        && !label.is_empty()
    {
        return label;
    }
    activity_label(id).to_string()
}
pub fn activity_label(id: &str) -> &'static str {
    match id {
        "projects" => "Projects",
        "tree" => "Folder tree",
        "search" => "Search",
        "problems" => "Problems",
        "terminal" => "Terminal",
        "extensions" => "Extensions",
        "customize" => "Customize",
        "claudeBridgePlan" => "Bridge Plan",
        "claudeBridgeTodos" => "Bridge Todos",
        "claudeBridgeAgents" => "Bridge Agents",
        "claudeBridgeToolsUsage" => "Bridge Tools",
        _ => "",
    }
}
/// Тот же глиф, но с ховером ГРУППЫ-родителя: в оригинале
/// `.btn:hover { color: var(--text-primary) }`, а иконка красится
/// `fill="currentColor"`, поэтому светлеет вместе с плиткой. В gpui цвет
/// прибит аргументом и `.hover()` родителя до него не доходит — нужен
/// `group_hover` (ревью ц.15).
pub fn tool_glyph_group_hover(
    icon: &str,
    svg_px: f32,
    codicon_px: f32,
    color: gpui::Rgba,
    group: SharedString,
    hover: gpui::Rgba,
) -> AnyElement {
    if let Some(path) = phosphor_path(icon) {
        return svg()
            .path(SharedString::from(path))
            .w(px(svg_px))
            .h(px(svg_px))
            .text_color(color)
            .group_hover(group, move |s| s.text_color(hover))
            .into_any_element();
    }
    if icon.starts_with("file:")
        || icon.starts_with('/')
        || icon.starts_with("http")
        || icon.starts_with("data:")
    {
        // Картиночная ветка цвета не имеет — отдаём как есть
        return tool_glyph_split(icon, svg_px, codicon_px, color);
    }
    crate::ui::icon::codicon_str(codicon_glyph(icon), codicon_px)
        .text_color(color)
        .group_hover(group, move |s| s.text_color(hover))
        .into_any_element()
}
/// То же, но с РАЗНЫМИ кеглями веток: у оригинала svg-иконка тула — 18
/// (`DEFAULT_SIZE_PX`, `ToolIcon.tsx:24`), а codicon без переопределения в
/// модуле берёт базу `.codicon { font-size: 16px }` (`codicon.css:13`).
/// Там, где модуль кегль задаёт (бар 18, стрип 13), обе ветки равны.
pub fn tool_glyph_split(icon: &str, svg_px: f32, codicon_px: f32, color: gpui::Rgba) -> AnyElement {
    // Ветка `<img>` для иконок расширения (`isImageIcon`, `activity.ts:89`).
    // `gpui::img(String)` считает URI ВСЁ, включая `file:`/`/path`, и уходит в
    // HTTP GET — путь к файлу надо отдавать как `PathBuf` (ревью ц.13).
    // `file:///C:/x` → `C:/x`, а абсолютный путь отдаём ЦЕЛИКОМ: срез
    // ведущего слэша делал его относительным (ревью ц.14)
    if let Some(path) = icon
        .strip_prefix("file:///")
        // `file://host/share/x` и `file://C:/x` — тоже файловые пути
        .or_else(|| icon.strip_prefix("file://"))
        .or_else(|| icon.strip_prefix("file:"))
        .or_else(|| icon.starts_with('/').then_some(icon))
    {
        return gpui::img(std::path::PathBuf::from(path))
            .w(px(svg_px))
            .h(px(svg_px))
            .into_any_element();
    }
    // `data:image/...;base64,...` — декодируем сами и отдаём картинкой
    if let Some(img) = crate::ui::icon::data_uri_image(icon) {
        return gpui::img(img)
            .w(px(svg_px))
            .h(px(svg_px))
            .into_any_element();
    }
    if icon.starts_with("http:") || icon.starts_with("https:") {
        return gpui::img(icon.to_string())
            .w(px(svg_px))
            .h(px(svg_px))
            .into_any_element();
    }
    // `data:`-иконки движок не декодирует — падают в codicon-фолбэк
    match phosphor_path(icon) {
        Some(path) => svg()
            .path(SharedString::from(path))
            .w(px(svg_px))
            .h(px(svg_px))
            .text_color(color)
            .into_any_element(),
        // Бокс = кегль: без него line-box кодикона тянется до 1.169·кегль
        // (в браузере `.codicon` — `16px/1`), и глиф уезжает вниз
        None => crate::ui::icon::codicon_str(codicon_glyph(icon), codicon_px)
            .text_color(color)
            .into_any_element(),
    }
}
