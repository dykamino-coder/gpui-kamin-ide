//! Правая часть титлбара: поиск команд, быстрые действия, контролы окна.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::probe::registry::probe_area;
use crate::ui::focus_ring::FocusRing;
use crate::ui::icon::{
    CHROME_CLOSE, CHROME_MAXIMIZE, CHROME_MINIMIZE, CHROME_RESTORE, FA_BUG, FA_TABLE_COLUMNS,
    SEARCH, codicon, fa,
};
use crate::ui::titlebar::TitlebarState;
use crate::ui::titlebar_buttons::action_button;
use crate::ui::titlebar_buttons::control_button;
use gpui::prelude::*;
use gpui::{AnyElement, Window, WindowControlArea, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Кнопка поиска команд (палитра).
#[allow(clippy::too_many_arguments)]
/// Поиск команд и быстрые действия (раскладка, оформление) —
/// три соседних ребёнка титлбара в прежнем порядке.
pub(crate) fn search_and_actions(
    p: &Palette,
    state: &TitlebarState,
    window: &Window,
    open_palette: Box<dyn Fn()>,
) -> Vec<AnyElement> {
    let _ = window;
    let search_bg = tint(rgba(p.bg_surface), 0.6);
    let search_border = tint(rgba(p.bg_overlay), 0.3);
    vec![
        div()
            .id("command-search")
            .occlude()
            .relative()
            .focus_ring("tb:command-search", m::RADIUS_MD, rgba(p.accent_primary))
            .child(probe_area("command-search"))
            .flex()
            .items_center()
            .gap(px(m::SPACE_2))
            .h(px(26.0))
            .px(px(m::SPACE_3))
            .mr(px(m::SPACE_2))
            .flex_shrink_0()
            .rounded(px(m::RADIUS_SM))
            .bg(search_bg)
            .border_1()
            .border_color(search_border)
            .text_size(px(m::FS_XS))
            .text_color(rgba(p.text_muted))
            .cursor_pointer()
            .hover(move |s| s.bg(rgba(p.bg_surface)).text_color(rgba(p.text_secondary)))
            .tooltip(crate::ui::tooltip::tooltip(
                "Open command palette (Ctrl+Shift+P)",
            ))
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| open_palette())
            .child(codicon(SEARCH, 12.0))
            // .searchHint padding 0 8
            .child(div().px(px(m::SPACE_2)).child("Type a command…"))
            .into_any_element(),
        action_button(
            "layout-toggles",
            26.0,
            m::RADIUS_MD,
            "Layout panels",
            p,
            state.layout_popover_open,
            false,
            {
                let tx = state.tx.clone();
                move |_, _| {
                    let _ = tx.try_send(crate::host_link::ShellEvent::ToggleLayoutPopover);
                }
            },
            fa(FA_TABLE_COLUMNS, 13.0).into_any_element(),
        )
        .into_any_element(),
        action_button(
            "theme-toggle",
            m::ICON_BUTTON_ROUND, // ThemeQuickToggle .trigger 28px
            m::RADIUS_SM,
            "Appearance — themes & icons",
            p,
            false,
            true,
            {
                let tx = state.tx.clone();
                move |_, _| {
                    let _ = tx.try_send(crate::host_link::ShellEvent::ToggleAppearancePopover);
                }
            },
            fa(state.theme_glyph, 12.0).into_any_element(),
        )
        .into_any_element(),
    ]
}
/// Контролы окна: DevTools и min/max/close.
#[allow(clippy::too_many_arguments)]
pub(crate) fn window_controls(p: &Palette, state: &TitlebarState, window: &Window) -> AnyElement {
    let maximize_glyph = if window.is_maximized() {
        CHROME_RESTORE
    } else {
        CHROME_MAXIMIZE
    };
    // Контролы: DevTools (fa-bug + label, radius 12) + min/max/close
    div()
        .flex()
        .items_center()
        .h_full()
        .pr(px(m::SPACE_1))
        .child(
            div()
                .id("devtools")
                .occlude()
                .focus_ring("tb:devtools", m::RADIUS_MD, rgba(p.accent_primary))
                .h(px(m::ICON_BUTTON_TITLEBAR))
                .px(px(m::SPACE_3))
                .mx(px(m::SPACE_1))
                .flex()
                .items_center()
                .gap(px(m::SPACE_1))
                .rounded(px(m::RADIUS_MD))
                .cursor_pointer()
                .text_color(rgba(p.text_muted))
                .hover(move |s| s.bg(rgba(p.bg_surface)).text_color(rgba(p.accent_primary)))
                .tooltip(crate::ui::tooltip::tooltip("DevTools"))
                .on_mouse_down(gpui::MouseButton::Left, {
                    let tx = state.tx.clone();
                    move |_, _, cx| {
                        cx.stop_propagation();
                        let _ = tx.try_send(crate::host_link::ShellEvent::TitlebarDevtools);
                    }
                })
                .child(
                    // `.btn > i { width:16px; height:16px; font-size:13px }`
                    // (0,1,1) перебивает FA-шное `width: 1.25em` —
                    // бокс ровно 16×16 (ревью ц.19)
                    fa(FA_BUG, 13.0).w(px(16.0)).h(px(16.0)),
                )
                .child(div().text_size(px(m::FS_SM)).child("DevTools")),
        )
        .child(control_button(
            "win-min",
            CHROME_MINIMIZE,
            "Minimize",
            p,
            WindowControlArea::Min,
            false,
        ))
        .child(control_button(
            "win-max",
            maximize_glyph,
            if window.is_maximized() {
                "Restore"
            } else {
                "Maximize"
            },
            p,
            WindowControlArea::Max,
            false,
        ))
        .child(control_button(
            "win-close",
            CHROME_CLOSE,
            "Close",
            p,
            WindowControlArea::Close,
            true,
        ))
        .into_any_element()
}
