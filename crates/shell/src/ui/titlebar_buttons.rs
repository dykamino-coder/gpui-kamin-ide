//! Кнопки титлбара: системные контролы и действия.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::probe::registry::probe_area;
use crate::ui::focus_ring::FocusRing;
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{AnyElement, App, SharedString, Window, WindowControlArea, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Оконный контрол: 36×36 круг (TitlebarButton.module.css).
pub(crate) fn control_button(
    id: &'static str,
    glyph: &'static str,
    tip: &'static str,
    p: &Palette,
    area: WindowControlArea,
    danger: bool,
) -> impl IntoElement {
    let (hover_bg, hover_fg) = if danger {
        (rgba(p.accent_red), rgba(p.bg_primary))
    } else {
        (rgba(p.bg_surface), rgba(p.text_primary))
    };
    div()
        .id(id)
        .relative()
        .child(probe_area(id))
        // occlude: перекрывает корневой Drag-hitbox титлбара (иначе NCHITTEST
        // вернул бы HTCAPTION и клик утёк бы в драг окна)
        .occlude()
        .window_control_area(area)
        .w(px(m::ICON_BUTTON_TITLEBAR))
        .h(px(m::ICON_BUTTON_TITLEBAR))
        .mx(px(m::SPACE_1))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .focus_ring(
            &format!("tbc:{id}"),
            m::ICON_BUTTON_TITLEBAR / 2.0,
            rgba(p.accent_primary),
        )
        .text_color(rgba(p.text_muted))
        .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
        .tooltip(crate::ui::tooltip::tooltip(tip))
        .on_mouse_down(gpui::MouseButton::Left, move |_, window, _cx| match area {
            WindowControlArea::Min => window.minimize_window(),
            // `zoom_window()` на Windows умеет ТОЛЬКО разворачивать
            WindowControlArea::Max => crate::overlay::toggle_main_maximize(),
            WindowControlArea::Close => window.remove_window(),
            WindowControlArea::Drag => {}
        })
        // Замер оригинала (CDP): computed font-size глифа = 16px, ink
        // 10.4×11.2 лог.; при 13 у нас выходило 6.4×8.0 — заметно мельче
        .child(codicon(glyph, 16.0))
}
/// Иконка-действие титлбара. Точные размеры per-кнопка:
/// quick-action/theme 28×28 r8; LayoutToggles 26×26 r12 (их CSS различается).
#[allow(clippy::too_many_arguments)]
pub(crate) fn action_button(
    id: &'static str,
    size: f32,
    radius: f32,
    tip: &'static str,
    p: &Palette,
    active: bool,
    muted: bool,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
    child: AnyElement,
) -> impl IntoElement {
    let hover_bg = rgba(p.bg_surface);
    let mut btn = div()
        .id(id)
        // Группа для `group_hover` детей: у SVG цвет задан аргументом и на
        // `.hover()` кнопки не реагирует (ревью ц.19)
        .group(SharedString::from(format!("qa-{id}")))
        .occlude() // не отдаёт клик корневому Drag титлбара
        // relative + probe_area: bounds триггера уходят в реестр, поповеры
        // считают анкор от НИХ (в оригинале — `getBoundingClientRect`
        // анкора), а не от зашитых офсетов
        .relative()
        .child(probe_area(id))
        .w(px(size))
        .h(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius))
        .cursor_pointer()
        // quickAction = text-secondary; theme .trigger = text-muted (ревью ц.2)
        .text_color(rgba(if muted {
            p.text_muted
        } else {
            p.text_secondary
        }))
        .hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
        .tooltip(crate::ui::tooltip::tooltip(tip))
        .on_mouse_down(gpui::MouseButton::Left, move |_, w, cx| on_click(w, cx))
        .child(child);
    if active {
        btn = btn
            .bg(tint(rgba(p.accent_primary), 0.16))
            .text_color(rgba(p.text_primary));
    }
    crate::ui::focus_ring::focusable(btn, &format!("tb:{id}"), radius, rgba(p.accent_primary))
}
