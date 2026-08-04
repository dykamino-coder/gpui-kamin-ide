//! Части тулбара терминала: вогнутый угол, кнопки прокрутки, ширина меню.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::host::events::TermEvent;
use crate::host_link::ShellEvent;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// Вогнутый уголок 6×6 активного таба (::before/::after оригинала):
/// квадрат минус четверть круга цветом editor-bg; дуга полилинией
/// (curve_to с ctrl в углу вырождает заливку — гоча glint.rs).
pub(crate) fn concave_corner(color: gpui::Rgba, left_side: bool) -> AnyElement {
    const R: f32 = 6.0;
    div()
        .absolute()
        .bottom_0()
        .w(px(R))
        .h(px(R))
        .map(|d| {
            if left_side {
                d.left(px(-R))
            } else {
                d.right(px(-R))
            }
        })
        .child(
            gpui::canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let bx = f32::from(bounds.origin.x);
                    let by = f32::from(bounds.origin.y);
                    // Угол заливки и знаки к центру дуги (top-inner угол бокса)
                    let (cx_, cy_, sx, sy) = if left_side {
                        (bx + R, by + R, -1.0_f32, -1.0_f32)
                    } else {
                        (bx, by + R, 1.0, -1.0)
                    };
                    let a = gpui::point(px(cx_), px(cy_));
                    let b = gpui::point(px(cx_ + sx * R), px(cy_));
                    let (cxc, cyc) = (cx_ + sx * R, cy_ + sy * R);
                    let mut path = gpui::Path::new(a);
                    path.line_to(b);
                    for k in 1..=12 {
                        let t = k as f32 / 12.0 * std::f32::consts::FRAC_PI_2;
                        path.line_to(gpui::point(
                            px(cxc - sx * R * t.sin()),
                            px(cyc - sy * R * t.cos()),
                        ));
                    }
                    path.line_to(a);
                    window.paint_path(path, color);
                },
            )
            .absolute()
            .size_full(),
        )
        .into_any_element()
}
/// Полоса: табы живых шеллов + «+» (дропдаун — deferred под кнопкой).
/// Ширина пилюли таба (иконка+лейбл+×) для расчёта видимого окна.
pub(crate) const TAB_W: f32 = 112.0;
/// Шеврон прокрутки окна табов (TerminalToolbar .scrollBtn).
pub(crate) fn scroll_btn(
    id: &'static str,
    glyph: &'static str,
    enabled: bool,
    delta: i32,
    p: &Palette,
    tx: &Sender<ShellEvent>,
) -> AnyElement {
    // .scrollBtn: 22×30, radius-xs, text-secondary, hover bg-surface
    let mut b = div()
        .id(id)
        .w(px(22.0))
        .h(px(30.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(m::RADIUS_XS))
        .text_color(rgba(p.text_secondary))
        .child(crate::ui::icon::codicon(glyph, 12.0));
    if enabled {
        let tx = tx.clone();
        let hb = rgba(p.bg_surface);
        b = b
            .cursor_pointer()
            .hover(move |s| s.bg(hb).text_color(rgba(p.text_primary)))
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::Term(TermEvent::TermTabScroll(delta)));
            });
    } else {
        b = b.opacity(0.35);
    }
    crate::ui::focus_ring::focusable(
        b,
        &format!("term:{id}"),
        m::RADIUS_XS,
        rgba(p.accent_primary),
    )
    .into_any_element()
}
#[allow(clippy::too_many_arguments)]
/// До первого замера якоря держим прежнее поведение: правый край меню на
/// правом крае кнопки (кнопка 28 шириной).
pub(crate) fn aw_fallback(menu_w: f32) -> f32 {
    28.0 - menu_w
}
