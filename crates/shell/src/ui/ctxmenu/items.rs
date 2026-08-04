//! Пункты меню сессии: строка, образец цвета, RPC-обёртка.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{parse_hex, rgba, tint};
use crate::host_link::{self, ShellEvent};
use crate::ui::ctxmenu::colors::resolve_session_color;
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use serde_json::json;
use smol::channel::Sender;

pub(crate) fn rpc_then_close(
    tx: &Sender<ShellEvent>,
    method: &'static str,
    args: Vec<serde_json::Value>,
) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        if let Some(client) = host_link::client() {
            let _ = client.request(method, args);
        }
        let _ = tx.try_send(ShellEvent::CloseSessionMenu);
    });
}
pub(crate) fn menu_item(
    id: &str,
    glyph: &'static str,
    label: impl Into<SharedString>,
    danger: bool,
    p: &Palette,
    on_click: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let (base_fg, hover_bg, hover_fg) = if danger {
        (
            rgba(p.accent_red),
            tint(rgba(p.accent_red), 0.16),
            rgba(p.accent_red),
        )
    } else {
        (
            rgba(p.text_secondary),
            tint(rgba(p.text_primary), 0.10),
            rgba(p.text_primary),
        )
    };
    let row = div()
        .id(SharedString::from(id.to_string()))
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .w_full()
        .px(px(m::SPACE_2))
        .py(px(6.0))
        .rounded(px(m::RADIUS_SM))
        .text_size(px(m::FS_SM))
        .text_color(base_fg)
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
        .on_mouse_down(gpui::MouseButton::Left, move |_, w, cx| {
            cx.stop_propagation();
            on_click(w, cx);
        })
        .child(codicon(glyph, 14.0))
        .child(label.into());
    // Пункты меню оригинала — `<button role="menuitem">`, значит таб-стопы с
    // `button:focus-visible` (`theme/global.css:38-43`). Кольцо в ОБЩЕМ
    // конструкторе — его получают все пункты сразу (ревью ц.26)
    crate::ui::focus_ring::focusable(row, id, m::RADIUS_SM, rgba(p.accent_primary))
        .into_any_element()
}
pub(crate) fn swatch(
    id: &str,
    color: &'static str,
    active: bool,
    sid: String,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // Свотч показывает вариант ТЕКУЩЕЙ темы, хранится всегда dark-значение
    // (`resolveSessionColor`, `sessions.ts:34-37`) — ревью ц.13
    let fill = parse_hex(resolve_session_color(color), rgba(p.accent_primary));
    let border = if active {
        rgba(p.text_primary)
    } else {
        tint(rgba(p.text_primary), 0.0)
    };
    let tx = tx.clone();
    div()
        .id(SharedString::from(id.to_string()))
        .w(px(16.0))
        .h(px(16.0))
        .rounded_full()
        .border_2()
        .border_color(border)
        .bg(fill)
        .cursor_pointer()
        // `transform: scale(1.15)` оригинала в gpui недоступен; выдуманного
        // ховера лучше не иметь совсем (ревью ц.8)
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            rpc_then_close(
                &tx,
                "kamin:sessions:setColor",
                vec![json!(sid), json!(color)],
            );
        })
        .into_any_element()
}
