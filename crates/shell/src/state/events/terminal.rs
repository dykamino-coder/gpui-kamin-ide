//! Терминал: профили, ввод, скролл, меню профилей, закрытие вкладок.
//!
//! Кого сюда зовут — решает `state/events/dispatch.rs`; связку
//! «вариант события → модуль» проверяет `scripts/check_event_routing.py`.

use crate::host::events::TermEvent;
use crate::host_link::ShellEvent;
use gpui::{Context, px};

use crate::root::RootView;

impl RootView {
    /// Терминал: профили, ввод, скролл, меню профилей, закрытие вкладок.
    pub(crate) fn apply_terminal(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::Term(TermEvent::TermWakeup) => {} // cx.notify делает насос — грид перерисуется
            ShellEvent::Term(TermEvent::TermInput(s)) => {
                if let Some(t) = self.term.terminals.get_mut(self.term.term_active) {
                    t.write(s.as_bytes());
                }
            }
            ShellEvent::Term(TermEvent::TermTabScroll(delta)) => {
                // Шеврон листает СТРАНИЦУ: `page = max(32, floor(clientWidth
                // × 0.8))`, `scrollLeft ± page` с клампом по `scrollWidth`
                // (`TerminalToolbar.tsx:70-79`)
                const SCROLL_PAGE_RATIO: f32 = 0.8;
                const SCROLL_MIN_PAGE_PX: f32 = 32.0;
                let view_w = f32::from(self.term.term_tab_scroll.bounds().size.width);
                let page = (view_w * SCROLL_PAGE_RATIO).floor().max(SCROLL_MIN_PAGE_PX);
                let max = f32::from(self.term.term_tab_scroll.max_offset().width);
                let cur = -f32::from(self.term.term_tab_scroll.offset().x);
                let next = (cur + page * delta as f32).clamp(0.0, max);
                self.term
                    .term_tab_scroll
                    .set_offset(gpui::point(px(-next), self.term.term_tab_scroll.offset().y));
            }
            ShellEvent::Term(TermEvent::TermSetDefaultShell(id)) => {
                crate::layout_store::save_patch(serde_json::json!({ "defaultShellId": &id }));
                self.term.term_default_shell = Some(id);
            }
            ShellEvent::Term(TermEvent::TermNew(profile_id)) => {
                self.term.term_menu_open = false;
                self.spawn_shell(crate::term::profile_by_id(&profile_id));
            }
            ShellEvent::Term(TermEvent::TermScroll(n)) => {
                if let Some(t) = self.term.terminals.get_mut(self.term.term_active) {
                    t.scroll(n);
                }
            }
            ShellEvent::Term(TermEvent::TermSelect(i)) if i < self.term.terminals.len() => {
                self.term.term_active = i;
            }
            ShellEvent::Term(TermEvent::TermClose(i)) => {
                if i < self.term.terminals.len() {
                    self.term.terminals[i].kill();
                    self.term.terminals.remove(i);
                    if self.term.term_active >= self.term.terminals.len() {
                        self.term.term_active = self.term.terminals.len().saturating_sub(1);
                    }
                }
            }
            ShellEvent::Term(TermEvent::ToggleTermMenu) => {
                self.term.term_menu_open = !self.term.term_menu_open
            }
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
