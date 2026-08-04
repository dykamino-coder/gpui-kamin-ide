//! BrowserPane (Web-режим файловой панели): навбар (back/forward/reload +
//! адресная строка) над страницей CEF. Навигация — через `web::*`.

use gpui::prelude::*;
use gpui::{AnyElement, App, Entity, div, px};
use gpui_component::input::{Input, InputState};
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::{rgba, tint};
use crate::ui::icon::codicon;

const ARROW_LEFT: &str = "\u{ea9b}";
const ARROW_RIGHT: &str = "\u{ea9c}";
const REFRESH: &str = "\u{eb37}";

/// Нормализация ввода адреса: схема как есть; «домен.tld» → https://; иначе
/// поиск Google.
pub fn normalize_url(input: &str) -> String {
    let v = input.trim();
    if v.is_empty() {
        return "about:blank".into();
    }
    if v.starts_with("http://") || v.starts_with("https://") || v.starts_with("about:") {
        return v.to_string();
    }
    if !v.contains(' ') && v.contains('.') {
        return format!("https://{v}");
    }
    format!("https://www.google.com/search?q={}", v.replace(' ', "+"))
}

fn nav_btn(
    id: &'static str,
    glyph: &'static str,
    tip: &'static str,
    p: &Palette,
    on_click: impl Fn(&mut App) + 'static,
) -> AnyElement {
    // .navBtn:hover — bg-surface-hover (не tint text-primary)
    let hover = rgba(p.bg_surface_hover);
    let b = div()
        .id(id)
        .w(px(26.0))
        .h(px(26.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(m::RADIUS_SM))
        .text_color(rgba(p.text_secondary))
        .cursor_pointer()
        .hover(move |s| s.bg(hover).text_color(rgba(p.text_primary)))
        .tooltip(crate::ui::tooltip::tooltip(tip))
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| on_click(cx))
        // codicon базовые 16 (ревью ц.1)
        .child(codicon(glyph, 16.0));
    crate::ui::focus_ring::focusable(
        b,
        &format!("nav:{id}"),
        m::RADIUS_SM,
        rgba(p.accent_primary),
    )
    .into_any_element()
}

/// Навбар браузерной панели: адресная строка и кнопки навигации. Страницу
/// рисует CEF, поэтому действия идут через `web::*`.
#[cfg(windows)]
pub fn visual_frame(
    address: &Entity<InputState>,
    addr_focused: bool,
    p: &Palette,
    viewport: AnyElement,
) -> AnyElement {
    let addr = address.clone();
    div()
        .id("browser-pane")
        .flex_1()
        .min_h(px(0.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(m::SPACE_1))
                .flex_shrink_0()
                .py(px(4.0))
                .px(px(6.0))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {
                    // Фокус вернётся хосту сам: страница живёт элементом кадра.
                })
                .child(nav_btn("br-back", ARROW_LEFT, "Back", p, |_| {
                    crate::web::go_back("browser");
                }))
                .child(nav_btn("br-fwd", ARROW_RIGHT, "Forward", p, |_| {
                    crate::web::go_forward("browser");
                }))
                .child(nav_btn("br-reload", REFRESH, "Reload", p, |_| {
                    crate::web::reload("browser");
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .px(px(10.0))
                        .h(px(26.0))
                        .flex()
                        .items_center()
                        .rounded(px(m::RADIUS_SM))
                        .bg(rgba(p.bg_base))
                        .border_1()
                        .border_color(if addr_focused {
                            rgba(p.accent_primary)
                        } else {
                            tint(rgba(p.text_primary), 0.06)
                        })
                        .text_size(px(m::FS_SM))
                        .text_color(rgba(p.text_primary))
                        .on_key_down(move |ev: &gpui::KeyDownEvent, _, cx| {
                            if ev.keystroke.key.as_str() == "enter" {
                                let url = normalize_url(&addr.read(cx).value());
                                crate::web::navigate("browser", &url);
                            }
                        })
                        .child(Input::new(address).appearance(false)),
                ),
        )
        // .viewport: инсет 6px по бокам/снизу (кадр редактора)
        .child(
            div()
                .flex_1()
                .min_h(px(0.))
                .px(px(6.0))
                .pb(px(6.0))
                .flex()
                // Подложку под догоняющий relayout даёт backdrop-визуал
                // (dcomp, ниже вебвью) — фон тут НЕЛЬЗЯ: зона должна быть
                // прозрачной «дырой» в кадре gpui
                .child(viewport),
        )
        .into_any_element()
}
