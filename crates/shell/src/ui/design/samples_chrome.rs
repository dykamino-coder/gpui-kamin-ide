//! Семплы дизайн-системы: заголовок секции, статус-бар, иконки панели.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::colors::tint;
use crate::ui::design_panel::MONO;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Section header — тот же ландмарк, что PROJECTS/CUSTOMIZE в сайдбаре:
/// padding 8/12, fs-xs/500, text-muted, ss01
/// (`letter-spacing .08em` в gpui недоступен).
pub(crate) fn sample_section_header(p: &Palette) -> AnyElement {
    div()
        .px(px(12.0))
        .py(px(m::SPACE_2))
        .text_size(px(m::FS_XS))
        .font(crate::ui::typo::ss01(gpui::FontWeight::MEDIUM))
        .text_color(rgba(p.text_muted))
        .child("SECTION")
        .into_any_element()
}
/// Элементы статус-бара: тот же рецепт, что в `status_bar.rs`
/// (gap 4, px 8, r-xs, fs 11, глиф 12; ok/warn/brand-тона).
pub(crate) fn sample_status_items(p: &Palette) -> AnyElement {
    let item = |glyph: Option<&'static str>, text: &'static str, color: gpui::Rgba| {
        // `.item:hover` — bg-surface 60 % + text-primary; ховера у семпла не
        // было, хотя в оригинале он есть у каждого элемента (ревью ц.13)
        let hover_bg = tint(rgba(p.bg_surface), 0.6);
        let el = div()
            .id(SharedString::from(format!("smp-sbi-{text}")))
            .flex()
            .items_center()
            .gap(px(m::SPACE_1))
            .px(px(m::SPACE_2))
            .rounded(px(m::RADIUS_XS))
            .text_size(px(11.0))
            .text_color(color)
            // `button { cursor: pointer }` — глобальное правило скелета
            // (`theme/skeleton.css:25-31`), пилюли оригинала это `<button>`
            // (ревью ц.25: у нас была стрелка)
            .cursor_pointer()
            .hover(move |st| st.bg(hover_bg).text_color(rgba(p.text_primary)))
            .when_some(glyph, |d, g| d.child(crate::ui::icon::codicon(g, 12.0)))
            .child(text);
        // Пилюли оригинала — `<button>`, значит таб-стопы с
        // `button:focus-visible` (`theme/global.css:38-43`); у нас колец не
        // было вовсе (ревью ц.26)
        crate::ui::focus_ring::focusable(
            el,
            &format!("smp-sbi:{text}"),
            m::RADIUS_XS,
            rgba(p.accent_primary),
        )
    };
    div()
        .flex()
        .flex_wrap()
        .gap(px(m::SPACE_2))
        .child(item(Some("\u{ea71}"), "3 active", rgba(p.accent_green)))
        .child(item(Some("\u{ea6c}"), "2 failed", rgba(p.accent_yellow)))
        .child(item(None, "UTF-8", rgba(p.text_muted)))
        // `.brand { color: accent-primary; font-weight: 500 }`
        .child(
            item(None, "KaminIDE 0.0.1", rgba(p.accent_primary))
                .font_weight(gpui::FontWeight::MEDIUM),
        )
        .into_any_element()
}
/// Семейство panel-иконок: тот же SVG-набор, что у LayoutToggles и
/// плейсхолдера — рамка + подсвеченный слот, подпись под иконкой.
pub(crate) fn sample_panel_icons(p: &Palette) -> AnyElement {
    use crate::ui::panel_placeholder::SlotIcon;
    let slots: [(SlotIcon, &'static str); 8] = [
        // Набор и порядок дословно из `component-samples-extra.tsx:198-200`:
        // «main» был потерян, «bottom» — лишний (ревью ц.6)
        // `left` и `main` в `PanelIcon.tsx` рисуют ОДНУ фигуру (левый столбец),
        // поэтому обе подписи идут с одним вариантом enum
        (SlotIcon::Main, "left"),
        (SlotIcon::Main, "main"),
        (SlotIcon::MainBottom, "main-bottom"),
        (SlotIcon::Center, "center"),
        (SlotIcon::CenterBottom, "center-bottom"),
        (SlotIcon::Right, "right"),
        (SlotIcon::RightTop, "right-top"),
        (SlotIcon::RightBottom, "right-bottom"),
    ];
    // Иконки — прямые дети `.compInline` с gap space-2 (у нас стоял space-3)
    let mut row = div().flex().flex_wrap().gap(px(m::SPACE_2));
    for (slot, label) in slots {
        row = row.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.0))
                .text_color(rgba(p.text_secondary))
                .child(crate::ui::panel_placeholder::slot_glyph_small_colored(
                    slot,
                    rgba(p.text_secondary),
                ))
                .child(
                    // Подпись — `<code class="codeInline">` с кеглем 10:
                    // моно, accent-primary на подложке accent 10 %, p 1/6,
                    // radius-xs (у нас был обычный muted-текст, ревью ц.13)
                    div()
                        .font_family(MONO)
                        .text_size(px(10.0))
                        .px(px(6.0))
                        .py(px(1.0))
                        .rounded(px(m::RADIUS_XS))
                        .bg(tint(rgba(p.accent_primary), 0.10))
                        .text_color(rgba(p.accent_primary))
                        .child(label),
                ),
        );
    }
    row.into_any_element()
}
