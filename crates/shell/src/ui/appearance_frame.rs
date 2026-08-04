//! Каркас поповера оформления: шапка «Appearance», кнопка System, колонки.
//!
//! Блок перенесён как есть (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

#[allow(clippy::too_many_arguments)]
pub(crate) fn frame(
    dark_rows: Vec<AnyElement>,
    light_rows: Vec<AnyElement>,
    icon_rows: Vec<AnyElement>,
    w_dark: f32,
    w_light: f32,
    w_icons: f32,
    pop_w: f32,
    builtin_active: bool,
    theme_choice: &str,
    vw: f32,
    column: impl Fn(&'static str, Vec<AnyElement>, f32) -> AnyElement,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let system_on = builtin_active && theme_choice == "system";
    let sys_bg = if system_on {
        tint(rgba(p.accent_primary), 0.16)
    } else {
        tint(rgba(p.text_primary), 0.0)
    };
    let sys_hover = if system_on {
        tint(rgba(p.accent_primary), 0.16)
    } else {
        tint(rgba(p.text_primary), 0.10)
    };
    // Без pb: вертикальный ритм даёт gap(space-2) самого поповера (ревью ц.1)
    let header = div()
        .flex()
        .items_center()
        // `.header { gap: var(--space-3) }` — минимальный зазор
        // заголовок↔System (ревью ц.15)
        .gap(px(m::SPACE_3))
        .px(px(m::SPACE_1))
        .child(
            div()
                .flex_1()
                .text_size(px(m::FS_SM))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgba(p.text_primary))
                .child("Appearance"),
        )
        .child({
            let tx = tx.clone();
            div()
                .id("ap-system")
                .flex()
                .items_center()
                .gap(px(m::SPACE_2))
                .px(px(m::SPACE_2))
                .py(px(m::SPACE_1))
                .rounded(px(m::RADIUS_SM))
                .bg(sys_bg)
                .text_size(px(m::FS_XS))
                .text_color(rgba(if system_on {
                    p.text_primary
                } else {
                    p.text_muted
                }))
                .cursor_pointer()
                .tooltip(crate::ui::tooltip::tooltip(
                    "Follow the OS light/dark setting",
                ))
                .hover(move |s| s.bg(sys_hover).text_color(rgba(p.text_primary)))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = tx.try_send(ShellEvent::SetThemeChoice("system"));
                })
                .child(crate::ui::icon::fa("\u{f042}", 11.0).into_any_element())
                .child("System")
        });

    // Свой фрейм: right-анкор + ширина ПО КОНТЕНТУ (css .menu
    // width:max-content) — фиксированная ширина резала имена тем и
    // оставляла пустые полосы региона по краям.
    // `ThemeQuickToggle.module.css`: `.menu { position:absolute; top:
    // calc(100% + 4px); right: 0 }` от `.root` → правый край поповера ровно
    // на правом крае триггера, верх на 4 ниже него. Считаем от bounds
    // триггера, а не от зашитого офсета (был 250 → 11px мимо, ревью ц.8).
    let [tx_x, tx_y, tx_w, tx_h] =
        crate::probe::registry::bounds_of("theme-toggle").unwrap_or([vw - 250.0, 7.0, 28.0, 28.0]);
    div()
        .id("appearance-popover")
        .occlude()
        .absolute()
        .top(px(tx_y + tx_h + 4.0))
        // ⚠ ТОЛЬКО left-анкор. `.right()` резолвится к вьюпорту OVERLAY-окна,
        // который шире main, — поповер уезжал за правый край и обрезался
        // (регрессия, поймана ц.11: левый край 1316.8 при окне 1400).
        // Правый край держим на правом крае триггера через известную ширину.
        .left(px((tx_x + tx_w - pop_w).max(m::SPACE_2)))
        .w(px(pop_w))
        .flex()
        .flex_col()
        .gap(px(m::SPACE_2))
        .p(px(m::SPACE_2))
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_surface))
        .border_1()
        .border_color(tint(rgba(p.text_primary), 0.06))
        .shadow(crate::overlay::dropdown_shadow())
        .child(crate::overlay::hit_area())
        .child(header)
        .child(
            div()
                .flex()
                .gap(px(m::SPACE_2))
                .child(column("Dark", dark_rows, w_dark))
                .child(column("Light", light_rows, w_light))
                .child(column("Icons", icon_rows, w_icons)),
        )
        .into_any_element()
}
