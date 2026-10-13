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
/// Строка повторяет чип таба (владелец, 2026-10-13): слева пин (по ховеру,
/// у закреплённых — всегда), справа disconnect у открытых сессий.
pub fn tabs_overflow_menu(
    items: &[Session],
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
    for s in items {
        let (id, name) = (&s.id, &s.name);
        let dot = parse_hex(s.color.as_deref().unwrap_or("#89b4fa"), rgba(p.accent_blue));
        let group: SharedString = format!("ovm-g-{id}").into();
        let sid = id.clone();
        let item_hover = {
            let mut c = rgba(p.text_primary);
            c.a = 0.10;
            c
        };
        let mut row = div()
            .id(SharedString::from(format!("ovm-{id}")))
            .group(group.clone())
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
            // RMB → то же контекст-меню, что у чипа и строки сайдбара;
            // список закрываем, чтобы меню не легло под него
            .on_mouse_down(gpui::MouseButton::Right, {
                let tx = tx.clone();
                let data = crate::ui::sessions_list::menu_data(s);
                move |e: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    let _ = tx.try_send(ShellEvent::ToggleTabsOverflow(0.0, 0.0));
                    let _ = tx.try_send(ShellEvent::OpenSessionMenu(
                        data.clone(),
                        f32::from(e.position.x),
                        f32::from(e.position.y),
                    ));
                }
            })
            .child(
                div()
                    .w(px(16.0))
                    .h(px(16.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(crate::ui::tabs::chip_leading::leading(
                        s, p, dot, dot, false, false, &group, tx,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .child(div().w_full().truncate().child(name.clone())),
            );
        // Спящая (pinned+closed) — призрак, как у чипа
        if s.pinned && !s.open {
            row = row.opacity(0.55).text_color(rgba(p.text_muted));
        }
        if s.open {
            let id = id.clone();
            let tx = tx.clone();
            row = row.child(crate::ui::tabs::chip_action::chip_action(
                format!("ovmdc-{id}"),
                crate::ui::tabs::chip_action::DISCONNECT_GLYPH,
                "Disconnect (free from memory)",
                group,
                p,
                move || {
                    let _ = tx.try_send(ShellEvent::LocalSessionClosed(id.clone()));
                    let id = id.clone();
                    std::thread::spawn(move || {
                        if let Some(c) = crate::host_link::client() {
                            let _ =
                                c.request("kamin:sessions:deactivate", vec![serde_json::json!(id)]);
                        }
                    });
                },
                false,
            ));
        }
        menu = menu.child(row);
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
