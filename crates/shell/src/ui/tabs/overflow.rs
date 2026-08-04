//! Переполнение стрипа: спрятанные чипы, меню и кнопка.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{parse_hex, rgba};
use crate::host_link::ShellEvent;
use crate::ui::session_tabs::Tx;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_model::Session;
use kamin_theme::Palette;

/// id скрытых оверфлоу-кнопкой сессий (пишет session_tabs каждый рендер,
/// читает overlay-меню).
pub fn overflow_hidden_ids() -> &'static std::sync::Mutex<Vec<String>> {
    static S: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> = std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}
/// Оверфлоу-меню (в OVERLAY-окне): список скрытых сессий, клик = активация.
pub fn tabs_overflow_menu(
    items: &[(String, String, Option<String>)],
    x: f32,
    y: f32,
    viewport_w: f32,
    tx: &Tx,
    p: &Palette,
) -> gpui::AnyElement {
    let left = (x - 210.0).clamp(8.0, viewport_w - 248.0);
    let mut menu = div()
        .id("tabs-overflow-menu")
        .occlude()
        .absolute()
        .left(px(left))
        .top(px(y + 14.0))
        .w(px(240.0))
        .max_h(px(400.0))
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_surface))
        .border_1()
        .border_color({
            let mut c = rgba(p.text_primary);
            c.a = 0.06;
            c
        })
        .p(px(m::SPACE_1))
        .flex()
        .flex_col()
        .gap(px(1.0))
        .overflow_hidden()
        .child(crate::overlay::hit_area());
    for (id, name, color) in items {
        let dot = parse_hex(color.as_deref().unwrap_or("#89b4fa"), rgba(p.accent_blue));
        let sid = id.clone();
        let item_hover = {
            let mut c = rgba(p.text_primary);
            c.a = 0.10;
            c
        };
        menu = menu.child(
            div()
                .id(SharedString::from(format!("ovm-{id}")))
                .flex()
                .items_center()
                .gap(px(m::SPACE_2))
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_2))
                .rounded(px(m::RADIUS_SM))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_primary))
                .cursor_pointer()
                .hover(move |st| st.bg(item_hover))
                .on_mouse_down(gpui::MouseButton::Left, {
                    let tx = tx.clone();
                    move |_, _, cx| {
                        cx.stop_propagation();
                        let _ = tx.try_send(ShellEvent::ActivateSession(sid.clone()));
                        let _ = tx.try_send(ShellEvent::ToggleTabsOverflow(0.0, 0.0));
                    }
                })
                .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(dot))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .child(div().w_full().truncate().child(name.clone())),
                ),
        );
    }
    menu.into_any_element()
}
/// «N ⌄» + поповер со скрытыми сессиями (стиль дропдауна: bg-surface,
/// divider-soft, hover text-primary 10% — feedback_popover_surface).
pub fn overflow_button(
    hidden: &[&Session],
    open: bool,
    on_toggle: impl Fn(f32, f32) + 'static,
    p: &Palette,
) -> AnyElement {
    let hover_bg = rgba(p.bg_surface);
    let mut btn = div()
        .id("tabs-overflow")
        .occlude()
        .relative()
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(2.0))
        .h(px(28.0))
        .px(px(6.0))
        .ml(px(2.0))
        .rounded(px(m::RADIUS_MD))
        .text_size(px(12.0))
        .text_color(rgba(p.text_secondary))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
        .on_mouse_down(
            gpui::MouseButton::Left,
            move |e: &gpui::MouseDownEvent, _, _| {
                on_toggle(f32::from(e.position.x), f32::from(e.position.y))
            },
        )
        .child(format!("{}", hidden.len()))
        .child(
            div()
                .font_family("codicon")
                .text_size(px(12.0))
                .child(crate::ui::icon::CHEVRON_DOWN),
        );
    if open {
        btn = btn.bg(rgba(p.bg_surface)).text_color(rgba(p.text_primary));
    }
    crate::ui::focus_ring::focusable(btn, "tabs-overflow", m::RADIUS_SM, rgba(p.accent_primary))
        .into_any_element()
}
