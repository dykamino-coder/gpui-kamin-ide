//! Семплы: строка списка, поле ввода.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::colors::tint;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// `.itemList`/`.listItem` — паттерн строки сайдбара:
/// список gap 2 max-w 280; строка 8/12, r-sm, fs-md, text-secondary,
/// hover bg-surface 50% + primary; active accent 14% + accent-primary
/// (hover активной — 22%); disabled opacity .45.
pub(crate) fn sample_list_item(light: bool, p: &Palette) -> AnyElement {
    let rows: [(&'static str, &'static str, bool, bool); 4] = [
        ("\u{ea83}", "Sessions", false, false),
        ("\u{eb51}", "Settings (active)", true, false),
        ("\u{eae6}", "Extensions", false, false),
        ("\u{ead0}", "Disabled", false, true),
    ];
    let mut list = div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .w_full()
        .max_w(px(280.0));
    for (glyph, label, active, disabled) in rows {
        let hover_bg = tint(rgba(p.bg_surface), 0.5);
        let mut row = div()
            .id(label)
            .flex()
            .items_center()
            .gap(px(m::SPACE_2))
            .w_full()
            .px(px(m::SPACE_3))
            .py(px(m::SPACE_2))
            .rounded(px(m::RADIUS_SM))
            .text_size(px(m::FS_MD))
            .text_color(rgba(p.text_secondary))
            // `.listItem { cursor: pointer }`, `.listItemDisabled { cursor:
            // not-allowed }` — курсора у семпла не было вовсе (ревью ц.15)
            .cursor(if disabled {
                gpui::CursorStyle::OperationNotAllowed
            } else {
                gpui::CursorStyle::PointingHand
            })
            .child(crate::ui::icon::codicon(glyph, 14.0))
            .child(
                // `.tabLabel { overflow: hidden; text-overflow: ellipsis;
                // min-width: 0 }`
                div()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(label),
            );
        if active && light {
            // `[data-theme=light] .listItemActive` — сплошная заливка accent,
            // текст accent-action-fg, weight 600; ховер её сохраняет
            let fill = rgba(p.accent_primary);
            // `[data-theme=light] .listItemActive:hover { background:
            // var(--accent-action-hover) }` — ховер МЕНЯЕТ заливку, у нас
            // она держалась прежней (ревью ц.13)
            let fill_hover = rgba(p.accent_action_hover);
            row = row
                .bg(fill)
                .text_color(rgba(p.accent_action_fg))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .hover(move |s| s.bg(fill_hover));
        } else if active {
            row = row
                .bg(tint(rgba(p.accent_primary), 0.14))
                .text_color(rgba(p.accent_primary))
                .hover(move |s| s.bg(tint(rgba(p.accent_primary), 0.22)));
        } else if disabled {
            row = row.opacity(0.45);
        } else {
            row = row.hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)));
        }
        list = list.child(row);
    }
    list.into_any_element()
}
/// `.input` — 8/12, border bg-surface, r-sm, bg-base, fs-md, max-w 360;
/// `:focus` → рамка accent-primary. Это ЖИВОЙ `<input>` оригинала
/// (`component-samples.tsx:88-94`), а не статичный div (ревью ц.24).
pub(crate) fn sample_input(
    input: Option<&gpui::Entity<gpui_component::input::InputState>>,
    focused: bool,
    p: &Palette,
) -> AnyElement {
    let box_ = div()
        .w_full()
        .max_w(px(360.0))
        .px(px(m::SPACE_3))
        .py(px(m::SPACE_2))
        .rounded(px(m::RADIUS_SM))
        .bg(rgba(p.bg_base))
        .border_1()
        .border_color(rgba(if focused {
            p.accent_primary
        } else {
            p.bg_surface
        }))
        .text_size(px(m::FS_MD))
        // `color: var(--text-primary)` — цвет НАБРАННОГО текста
        .text_color(rgba(p.text_primary));
    match input {
        Some(state) => box_
            .child({
                use gpui_component::Sizable as _;
                gpui_component::input::Input::new(state)
                    .appearance(false)
                    // Input берёт кегль из своего Size (×0.875)
                    .with_size(gpui_component::Size::Size(px(m::FS_MD / 0.875)))
            })
            .into_any_element(),
        // До первого кадра страницы состояния ещё нет — рисуем плейсхолдер
        // цветом UA (`::placeholder` = currentColor 54 %)
        None => box_
            .text_color({
                let mut c = rgba(p.text_primary);
                c.a = 0.54;
                c
            })
            .child("Sample input")
            .into_any_element(),
    }
}
