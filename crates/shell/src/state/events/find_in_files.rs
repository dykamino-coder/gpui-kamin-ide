//! Обработка событий: панель «Найти в файлах» — открытие, закрытие, результаты.
//!
//! Вызывается диспетчером `state/events/dispatch.rs`. Состояние панели живёт
//! в `RootView.sov` рядом с остальными оверлеями поиска.

use crate::host_link::ShellEvent;
use gpui::Context;

use crate::root::RootView;

impl RootView {
    /// Панель «Найти в файлах»: открытие, закрытие, приём результатов.
    pub(crate) fn apply_find_in_files(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::ToggleFindInFiles => {
                self.sov.fif_open = !self.sov.fif_open;
                if !self.sov.fif_open {
                    self.sov.fif_input = None;
                    self.sov.fif_sub = None;
                    self.sov.fif_results.clear();
                    self.sov.fif_query_len = 0;
                    self.sov.fif_busy = false;
                }
            }
            ShellEvent::CloseFindInFiles => {
                self.sov.fif_open = false;
                self.sov.fif_input = None;
                self.sov.fif_sub = None;
                self.sov.fif_results.clear();
                self.sov.fif_query_len = 0;
                self.sov.fif_busy = false;
            }
            ShellEvent::FindInFilesResults(hits) => {
                self.sov.fif_results = hits;
                self.sov.fif_busy = false;
            }
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
