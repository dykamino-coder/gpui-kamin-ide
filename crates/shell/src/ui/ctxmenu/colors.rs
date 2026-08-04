//! Цвета сессии: пары и подбор ближайшего.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

/// Палитра цветов сессии (`SESSION_COLORS`, `sessions.ts:21-30`): пара
/// «тёмный вариант — светлый». В светлой теме пастель вымывается, поэтому
/// оригинал подменяет цвет насыщенным (`resolveSessionColor`).
pub const SESSION_COLOR_PAIRS: [(&str, &str); 8] = [
    ("#89b4fa", "#1e66f5"), // blue
    ("#a6e3a1", "#40a02b"), // green
    ("#f9e2af", "#df8e1d"), // yellow
    ("#fab387", "#fe640b"), // peach
    ("#f38ba8", "#d20f39"), // red
    ("#cba6f7", "#8839ef"), // mauve
    ("#94e2d5", "#179299"), // teal
    ("#f5c2e7", "#ea76cb"), // pink
];
/// Значения, которые ХРАНЯТСЯ у сессии (всегда dark-варианты).
pub const SESSION_COLORS: [&str; 8] = [
    "#89b4fa", "#a6e3a1", "#f9e2af", "#fab387", "#f38ba8", "#cba6f7", "#94e2d5", "#f5c2e7",
];
/// `resolveSessionColor`: в светлой теме — светлый вариант палитры, иначе
/// (и для непалитровых значений) цвет проходит как есть.
pub fn resolve_session_color(hex: &str) -> &str {
    if !kamin_theme::current_is_light() {
        return hex;
    }
    SESSION_COLOR_PAIRS
        .iter()
        .find(|(dark, _)| *dark == hex)
        .map(|(_, light)| *light)
        .unwrap_or(hex)
}
