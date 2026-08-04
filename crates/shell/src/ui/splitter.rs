//! Свои сплит-ручки (RightPanel.module.css 1:1):
//! Вертикальная: hit 8px в зазоре; по hover/drag — полоса 3px с растворением
//!   концов (transparent→tint-primary-strong 30..70%→transparent); idle пусто.
//! Горизонтальная: hit 10px, грип 32×3 r4 bg-overlay op .7; hover/drag —
//!   op 1 + accent-primary.
//! Hover — state-driven (RootView.hovered_handle) через on_hover; occlude НЕ
//! используем — mouse-up обязан пузыриться до корня (иначе drag «прилипает»).

use gpui::prelude::*;
use gpui::{AnyElement, MouseDownEvent, Window, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::{rgba, tint};

/// Полоса вертикального грипа: 3 сегмента (fade-in 30% / solid 40% / fade-out).
fn v_bar(color: gpui::Rgba, width: f32) -> AnyElement {
    let clear = tint(color, 0.0);
    div()
        .w(px(width))
        .h_full()
        .flex()
        .flex_col()
        .child(
            div()
                .h(gpui::relative(0.3))
                .w_full()
                .bg(gpui::linear_gradient(
                    180.,
                    gpui::linear_color_stop(clear, 0.0),
                    gpui::linear_color_stop(color, 1.0),
                )),
        )
        .child(div().h(gpui::relative(0.4)).w_full().bg(color))
        .child(
            div()
                .h(gpui::relative(0.3))
                .w_full()
                .bg(gpui::linear_gradient(
                    180.,
                    gpui::linear_color_stop(color, 0.0),
                    gpui::linear_color_stop(clear, 1.0),
                )),
        )
        .into_any_element()
}

/// Вертикальная ручка на стыке колонок: элемент нулевой ширины,
/// hit-зона 8px absolute поверх зазора.
pub fn v_handle(
    id: &'static str,
    p: &Palette,
    show: bool,
    // Идёт ЛЮБОЙ драг ручки: тултип глушится — мышь обгоняет ручку, и
    // «Drag to resize» мерцал появлением/пропаданием (жалоба юзера).
    dragging: bool,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
    on_hover: impl Fn(&bool, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let bar_color = tint(rgba(p.accent_primary), 0.25); // tint-primary-strong
    div()
        .relative()
        .w(px(0.))
        .h_full()
        .flex_shrink_0()
        .child(
            div()
                .id(id)
                .absolute()
                // Наш зазор эмулируется паддингами (4+4), стык врапперов =
                // ЦЕНТР зазора → left(-4) кладёт хит ровно на зазор (ревью
                // цикла 1: left(-8) залезал на кромку левой карты)
                .left(px(-4.0))
                .top_0()
                .w(px(m::SPACE_2))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_col_resize()
                .when(!dragging, |el| {
                    el.tooltip(crate::ui::tooltip::tooltip("Drag to resize"))
                })
                .on_mouse_down(gpui::MouseButton::Left, on_down)
                .on_hover(on_hover)
                .when(show, |el| el.child(v_bar(bar_color, 3.0))),
        )
        .into_any_element()
}

/// Горизонтальная ручка (стык верх/низ): высота 10px, грип 32×3 по центру.
/// `pr` — сдвиг грипа влево (правая колонка: паддинг под rail 48).
// Параметры — плоский набор примитивов конкретной ручки; собирать структуру
// ради одного колл-сайта на ручку дороже, чем длинная сигнатура.
#[allow(clippy::too_many_arguments)]
pub fn h_handle(
    id: &'static str,
    p: &Palette,
    show: bool,
    // См. v_handle: во время драга тултип глушится.
    dragging: bool,
    pr: f32,
    // Левый паддинг: у правой колонки обёртка шире карты на 4 (компенсация
    // `gap_wrap`), без парного отступа грип уезжает на 2px (ревью ц.14)
    pl: f32,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
    on_hover: impl Fn(&bool, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let grip = if show {
        div()
            .w(px(32.0))
            .h(px(3.0))
            .rounded(px(m::RADIUS_XS))
            .bg(rgba(p.accent_primary))
            .into_any_element()
    } else {
        div()
            .w(px(32.0))
            .h(px(3.0))
            .rounded(px(m::RADIUS_XS))
            .bg(rgba(p.bg_overlay))
            .opacity(0.7)
            .into_any_element()
    };
    div()
        .id(id)
        .flex_shrink_0()
        // .resizeHandle/.splitHandle { height: 10px }
        .h(px(10.0))
        // БЕЗ w_full: percent-ширина flex-col-ребёнка в gpui/taffy ломается
        // до интринсика (грип «уезжал» влево) — align-stretch растягивает сам
        .min_w(px(0.))
        .pl(px(pl))
        .pr(px(pr))
        .flex()
        .items_center()
        .justify_center()
        .cursor_row_resize()
        .when(!dragging, |el| {
            el.tooltip(crate::ui::tooltip::tooltip("Drag to resize"))
        })
        .on_mouse_down(gpui::MouseButton::Left, on_down)
        .on_hover(on_hover)
        .child(
            div()
                .relative()
                .child(crate::probe::registry::probe_area(id))
                .child(grip),
        )
        .into_any_element()
}
