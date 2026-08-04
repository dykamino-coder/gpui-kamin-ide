//! Панели кадра: терминал по требованию и вебвью браузера.
//!
//! Кусок `render` вынесен как есть (`plan/100-refactor-250.md`): порядок вызовов в кадре прежний.

use crate::state::model::RootView;
use gpui::prelude::*;
use gpui::{Context, Window};
use gpui_component::input::InputState;

impl RootView {
    pub(crate) fn frame_panels(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Терминал: лениво спавним шелл при первом показе terminal-тула
        let term_shown = crate::activity::PanelSlot::ALL
            .into_iter()
            .any(|s| self.activity.state(s).active.as_deref() == Some("terminal"));
        if term_shown && self.term.terminals.is_empty() {
            let def = self.term.term_default_shell.clone().unwrap_or_default();
            self.spawn_shell(crate::term::profile_by_id(&def));
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
