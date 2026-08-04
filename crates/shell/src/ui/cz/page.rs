//! Страница Customize: шапка, секции, contributed-обёртка.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Хедер страницы Customize: `h1` + подпись, общий для встроенных и
/// contributed-страниц (`CustomizePanel.tsx:33-36`).
pub fn page_header(title: SharedString, subtitle: SharedString, p: &Palette) -> gpui::Div {
    div()
        .flex_shrink_0()
        .pt(px(m::SPACE_5))
        .px(px(m::SPACE_6))
        .pb(px(m::SPACE_3))
        .border_b_1()
        .border_color(tint(rgba(p.bg_overlay), 0.3))
        .child(
            div()
                .text_size(px(m::FS_XL))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgba(p.text_primary))
                .child(title),
        )
        .child(
            div()
                .mt(px(m::SPACE_1))
                .text_size(px(m::FS_MD))
                .text_color(rgba(p.text_muted))
                .child(subtitle),
        )
}
/// Contributed-страница: тот же хедер, что у встроенных, и тело
/// `.bodyFlush` — flex 1, min-height 0, overflow hidden, БЕЗ паддингов
/// (`CustomizePanel.module.css:36-42`); вебвью красит себя от края до края.
pub fn contrib_page(name: &str, body: AnyElement, p: &Palette) -> AnyElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .min_h(px(0.))
        .child(page_header(
            SharedString::from(name.to_string()),
            SharedString::from("Contributed by an extension."),
            p,
        ))
        .child(
            div()
                .relative()
                .child(crate::probe::registry::probe_area("cz-body"))
                .flex_1()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(body),
        )
        .into_any_element()
}
pub fn title_for(id: &str) -> (&'static str, &'static str) {
    match id {
        // Тексты дословно из оригинала (`CustomizePanel.tsx:73-79`)
        "settings" => (
            "Settings",
            "All KaminIDE preferences and contributed configuration.",
        ),
        "design" => (
            "Design",
            "Theme tokens — colors, typography, spacing, radius, shadows, components.",
        ),
        "extensions" => (
            "Extensions",
            "Built-in and sideloaded extensions discovered at launch.",
        ),
        "logs" => ("Logs", "Output channels from the host and extensions."),
        "system" => (
            "System",
            "Host, extension and renderer diagnostics — errors and notifications.",
        ),
        _ => ("Coming soon", ""),
    }
}
/// `.section` — колонка с `gap: space-2`; заголовок `.sectionTitle`.
/// Вертикальный ритм между секциями даёт `.root { gap: space-4 }`, а не
/// margin у заголовка (из-за него у первой секции был лишний отступ 16,
/// а между заголовком и строкой — 4 вместо 8; ревью ц.13).
pub fn section(title: &'static str, rows: Vec<AnyElement>, p: &Palette) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(m::SPACE_2))
        .child(
            div()
                .relative()
                // Ширина титула — ТОЛЬКО замером региона: пиксельный поиск по
                // кадру трижды находил соседнюю строку (ц.32)
                .child(crate::probe::registry::probe_area("section-title"))
                // `.sectionTitle` — 11/600/uppercase/text-muted +
                // `letter-spacing: 0.06em` (`SettingsPanel.module.css:32`).
                // Трекинг появился вендорным патчем ц.31 (план 99):
                // 0.06em × 11 = 0.66 px
                .text_size(px(11.0))
                .letter_spacing(px(11.0 * 0.06))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgba(p.text_muted))
                .child(title.to_uppercase()),
        )
        .children(rows)
        .into_any_element()
}
