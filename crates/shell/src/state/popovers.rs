//! Закрытие поповеров, кроме одного.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::state::model::RootView;

impl RootView {
    /// Взаимоисключение поповеров: открытие одного закрывает остальные
    /// (каскады внутри одного поповера не считаются).
    pub(crate) fn close_popovers_except(&mut self, keep: &str) {
        if keep != "tabs" {
            self.tabs_overflow_open = None;
        }
        if keep != "session" {
            self.session_menu = None;
        }
        if keep != "file" {
            self.file_menu = None;
        }
        if keep != "picker" {
            self.tool_picker = None;
        }
        if keep != "etab" {
            self.ed.editor_tab_menu = None;
        }
        if keep != "layout" {
            self.layout_popover = false;
        }
        if keep != "appearance" {
            self.appearance_popover = false;
        }
        if keep != "newsession" {
            self.new_session_menu = None;
        }
        if keep != "ttab" {
            self.tool_tab_menu = None;
            self.tool_menu_sub = false;
        }
    }
}
