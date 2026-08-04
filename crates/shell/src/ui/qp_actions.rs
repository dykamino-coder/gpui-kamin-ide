//! Ряд Cancel/OK у мультивыборного Quick Pick.
//!
//! Вынесено из `quick_pick.rs` без изменения поведения
//! (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::qp_state::QuickPickState;
use gpui::prelude::*;
use gpui::{SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// `.actions`: justify-end, gap 8, 8/12, border-top bg-surface 60%.
pub(crate) fn actions(
    st: &QuickPickState,
    req_id: u64,
    tx: &Sender<ShellEvent>,
    p: &'static Palette,
) -> gpui::Div {
    let tx_ok = tx.clone();
    let checked: Vec<usize> = {
        let mut v: Vec<usize> = st.checked.iter().copied().collect();
        v.sort_unstable();
        v
    };
    let tx_cancel = tx.clone();
    let count = checked.len();
    // `.actions`: justify-end, gap 8, 8/12, border-top bg-surface 60%
    div()
        .flex()
        .justify_end()
        .gap(px(m::SPACE_2))
        .px(px(m::SPACE_3))
        .py(px(m::SPACE_2))
        .border_t_1()
        .border_color(tint(rgba(p.bg_surface), 0.6))
        .child(
            // `.cancelBtn`: прозрачная, text-secondary, hover bg-surface 60%
            div()
                .id("qp-cancel")
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_1))
                .rounded(px(m::RADIUS_SM))
                .border_1()
                .border_color(gpui::transparent_black())
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_secondary))
                .cursor_pointer()
                .hover(move |s| {
                    s.bg(tint(rgba(p.bg_surface), 0.6))
                        .text_color(rgba(p.text_primary))
                })
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = tx_cancel.try_send(ShellEvent::QuickPickResolve(req_id, None));
                })
                .child("Cancel"),
        )
        .child(
            div()
                .id("qp-ok")
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_1))
                .rounded(px(m::RADIUS_SM))
                // `border: 1px solid transparent` стоит у ОБЕИХ кнопок
                // (CSS:133-136); без него OK был на 2 px уже и ниже
                // Cancel, базовые линии текста разъезжались (ревью ц.25)
                .border_1()
                .border_color(gpui::transparent_black())
                // `.okBtn { background: var(--accent-primary) }`, ховер —
                // `--accent-action-hover`; своего начертания у кнопки нет
                // (ревью ц.14: стояли accent-action, SEMIBOLD и opacity .9)
                .bg(rgba(p.accent_primary))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.accent_action_fg))
                .cursor_pointer()
                .hover(move |s| s.bg(rgba(p.accent_action_hover)))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let _ =
                        tx_ok.try_send(ShellEvent::QuickPickResolve(req_id, Some(checked.clone())));
                })
                // `OK (N)` — счётчик выбранных пунктов
                // Счётчик печатается ВСЕГДА, включая «OK (0)»
                // (`QuickPickModal.tsx:119`, ревью ц.14)
                .child(SharedString::from(format!("OK ({count})"))),
        )
}
