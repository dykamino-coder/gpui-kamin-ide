//! Ведущая часть чипа сессии: точка состояния, кнопка пина, спиннер.
//!
//! Блок вынесен из `chip` как есть (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::session_tabs::Tx;
use crate::ui::sessions::glyphs::FA_THUMBTACK;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_model::Session;
use kamin_theme::Palette;

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
pub(crate) fn leading(
    s: &Session,
    p: &Palette,
    tab_color: gpui::Rgba,
    dot_color: gpui::Rgba,
    _is_active: bool,
    switching: bool,
    group: &SharedString,
    tx: &Tx,
) -> AnyElement {
    let pin_color = if s.pinned {
        tab_color
    } else {
        rgba(p.text_secondary)
    };
    let pin_hb = tint(tab_color, 0.16);
    let pin_tip = if s.pinned {
        "Unpin session"
    } else {
        "Pin session"
    };
    let pin_id = s.id.clone();
    let pinned = s.pinned;
    let pin_tx = tx.clone();
    div()
        .relative()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child({
            // `.switching .dot` — «дышит» 1→0.25→1 за 1s; анимируется
            // ТОЛЬКО точка, пин остаётся в слоте (ревью ц.19)
            use gpui::AnimationExt as _;
            let dot = div()
                .w(px(4.0))
                .h(px(4.0))
                .rounded_full()
                .bg(dot_color)
                .when(s.pinned, |d| d.invisible())
                .group_hover(group.clone(), |st| st.invisible());
            // Под RDP «дыхание» точки гонит кадры по сети — статичная точка.
            if switching && !crate::win_integration::reduce_motion() {
                dot.with_animation(
                    "chip-switching-pulse",
                    gpui::Animation::new(std::time::Duration::from_secs(1)).repeat(),
                    |d, delta| d.opacity(1.0 - 0.75 * (std::f32::consts::PI * delta).sin()),
                )
                .into_any_element()
            } else {
                dot.into_any_element()
            }
        })
        .child(
            div()
                .id(SharedString::from(format!("lead-pin-{}", s.id)))
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(m::RADIUS_XS))
                .text_color(pin_color)
                .cursor_pointer()
                .when(!s.pinned, |d| {
                    d.invisible().group_hover(group.clone(), |st| st.visible())
                })
                .hover(move |st| st.bg(pin_hb))
                .tooltip(crate::ui::tooltip::tooltip(pin_tip))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let _ =
                        pin_tx.try_send(ShellEvent::LocalSessionPinned(pin_id.clone(), !pinned));
                    let id = pin_id.clone();
                    std::thread::spawn(move || {
                        if let Some(c) = crate::host_link::client() {
                            let _ = c.request(
                                "kamin:sessions:setPinned",
                                vec![serde_json::json!(id), serde_json::json!(!pinned)],
                            );
                        }
                    });
                })
                .child(crate::ui::icon::fa(FA_THUMBTACK, 10.0)),
        )
        .into_any_element()
}
