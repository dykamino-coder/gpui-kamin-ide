//! Карточка веб-панели, когда CEF в этом процессе не поднялся (INC-2026-0076).
//!
//! Раньше такие панели вечно показывали скелет загрузки: «нет CEF» не
//! отличалось от «вью ещё грузится». Визуал — та же карточка, что у ошибки
//! загрузки (`webview_skeleton::load_error`): иконка, заголовок, подсказка и
//! кнопка в стиле Retry; действие — перезапуск приложения.

use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host::events::TermEvent;
use crate::host_link::ShellEvent;
use crate::web::{Outcome, Unavailable};

/// Заголовок и подсказка карточки по состоянию.
pub(crate) fn texts(state: Unavailable) -> (&'static str, String) {
    match state.outcome {
        Outcome::ProfileBusy(_) if !state.profile_free => (
            "Previous KaminIDE window is still closing",
            "Web panels will reload automatically when it exits.".into(),
        ),
        Outcome::ProfileBusy(_) => (
            "Web panels are ready to reload",
            "Save your changes, then restart KaminIDE.".into(),
        ),
        Outcome::Failed(code) => (
            "Web panels are unavailable",
            format!("The web engine did not start (code {code}). Restarting KaminIDE may help."),
        ),
        Outcome::Live => ("Web panels are unavailable", String::new()),
    }
}

pub fn card(state: Unavailable, tx: &Sender<ShellEvent>, p: &Palette) -> AnyElement {
    let tx = tx.clone();
    let (title, hint) = texts(state);
    let hover_bg = tint(rgba(p.text_primary), 0.12);
    div()
        .absolute()
        .inset(px(8.0))
        .rounded(px(m::RADIUS_MD))
        .overflow_hidden()
        .bg(rgba(p.editor_bg))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(m::SPACE_2))
        .p(px(24.0))
        .text_center()
        .child(crate::probe::registry::probe_area("web-unavailable"))
        .child(
            // fa-hourglass-half: ожидание, а не поломка
            div()
                .mb(px(m::SPACE_1))
                .font_family(crate::ui::icon::FA_FAMILY)
                .font_weight(gpui::FontWeight::BLACK)
                .text_size(px(22.0))
                .text_color(tint(rgba(p.accent_yellow), 0.85))
                .child("\u{f252}"),
        )
        .child(
            div()
                .text_size(px(m::FS_MD))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgba(p.text_primary))
                .child(title),
        )
        .child(
            div()
                .max_w(px(280.0))
                .text_size(px(m::FS_SM))
                .line_height(px(m::FS_SM * 1.4))
                .text_color(rgba(p.text_muted))
                .child(hint),
        )
        .child(
            div()
                .id("wv-restart")
                .relative()
                .flex()
                .items_center()
                .gap(px(6.0))
                .px(px(16.0))
                .py(px(6.0))
                .rounded(px(m::RADIUS_SM))
                .border_1()
                .border_color(tint(rgba(p.text_primary), 0.06))
                .bg(tint(rgba(p.text_primary), 0.06))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_primary))
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = tx.try_send(ShellEvent::Term(TermEvent::RestartForWeb(true)));
                })
                .child(crate::probe::registry::probe_area("wv-restart"))
                .child(
                    div()
                        .font_family(crate::ui::icon::FA_FAMILY)
                        .font_weight(gpui::FontWeight::BLACK)
                        .child("\u{f2f1}"),
                )
                .child("Restart now"),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(outcome: Outcome, profile_free: bool) -> Unavailable {
        Unavailable {
            outcome,
            profile_free,
        }
    }

    #[test]
    fn busy_profile_promises_automatic_reload() {
        let (title, hint) = texts(state(Outcome::ProfileBusy(24), false));
        assert_eq!(title, "Previous KaminIDE window is still closing");
        assert!(hint.contains("automatically"));
    }

    #[test]
    fn free_profile_with_unsaved_edits_asks_to_save() {
        let (_, hint) = texts(state(Outcome::ProfileBusy(21), true));
        assert!(hint.starts_with("Save your changes"));
    }

    #[test]
    fn other_failure_names_the_exit_code() {
        let (_, hint) = texts(state(Outcome::Failed(38), false));
        assert!(hint.contains("code 38"));
    }
}
