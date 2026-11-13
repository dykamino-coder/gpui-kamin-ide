//! Панели кадра: терминал по требованию и вебвью браузера.
//!
//! Кусок `render` вынесен как есть (`plan/100-refactor-250.md`): порядок вызовов в кадре прежний.

use crate::state::model::RootView;
use gpui::prelude::*;
use gpui::{Context, Window};
use gpui_component::input::InputState;

impl RootView {
    pub(crate) fn terminal_shown(&self) -> bool {
        terminal_shown(crate::activity::PanelSlot::ALL.into_iter().map(|slot| {
            (
                self.activity.state(slot).active.as_deref(),
                self.loader_slot_visible(slot),
            )
        }))
    }

    pub(crate) fn frame_panels(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Render only queues a request. Detection and PTY creation run elsewhere.
        if self.terminal_shown() && self.term.terminals.is_empty() && !self.term.spawn_pending {
            let id = self.term.term_default_shell.as_deref().unwrap_or_default();
            if crate::term::profile_by_id(id).is_some() {
                self.term.spawn_pending = self
                    .tx
                    .try_send(crate::host_link::ShellEvent::Term(
                        crate::host::events::TermEvent::TermEnsureShell,
                    ))
                    .is_ok();
            }
        }
        // Браузер Web-режима: строка адреса. Сама страница живёт в CEF и
        // заводится при отрисовке панели (`state/columns/file_web.rs`).
        if self.layout.file_panel_mode == "web" && self.browser_input.is_none() {
            self.browser_input = Some(cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(crate::web::DEMO_URL)
                    .placeholder("Search or enter address")
            }));
        }
    }
}

/// Visibility is part of the spawn contract, including parent panel visibility.
fn terminal_shown<'a>(mut slots: impl Iterator<Item = (Option<&'a str>, bool)>) -> bool {
    slots.any(|(active, visible)| visible && active == Some("terminal"))
}

#[cfg(test)]
mod tests {
    use super::terminal_shown;

    #[test]
    fn hidden_main_bottom_terminal_does_not_request_a_shell() {
        let active = Some("terminal");
        let mut main_bottom_visible = false;
        assert!(!terminal_shown([(active, main_bottom_visible)].into_iter()));
        main_bottom_visible = true;
        assert!(terminal_shown([(active, main_bottom_visible)].into_iter()));
    }

    #[test]
    fn only_a_visible_terminal_slot_requests_a_shell() {
        assert!(!terminal_shown(
            [(Some("terminal"), false), (Some("tree"), true)].into_iter()
        ));
        assert!(terminal_shown(
            [(Some("terminal"), false), (Some("terminal"), true)].into_iter()
        ));
        assert!(!terminal_shown([(None, true)].into_iter()));
    }
}
