//! Обработчики корня: действия, клавиши и мышь окна.
//!
//! Куски цепочки `render` вынесены как есть (`plan/100-refactor-250.md`):
//! обобщены по типу элемента, чтобы не зависеть от места в цепочке.

use crate::actions::{
    CloseOverlay, ToggleFindInFiles, TogglePalette, ToggleQuickOpen, ToggleWorkspaceSymbols,
};
use crate::host_link::{self, ShellEvent};
use crate::state::model::RootView;
use gpui::Context;

mod enter;

impl RootView {
    /// Действия и клавиши окна (палитра, поиск, Enter, Esc, undo).
    pub(crate) fn bind_actions<E>(&self, root: E, cx: &mut Context<Self>) -> E
    where
        E: gpui::InteractiveElement,
    {
        root.on_action(cx.listener(|this, _: &TogglePalette, _, cx| {
            this.palette_open = !this.palette_open;
            if this.palette_open {
                host_link::request_commands(this.tx.clone());
            } else {
                this.palette_input = None;
                this.palette_sub = None;
            }
            cx.notify();
        }))
        .on_action(cx.listener(|this, _: &ToggleQuickOpen, _, cx| {
            this.sov.quickopen_open = !this.sov.quickopen_open;
            if this.sov.quickopen_open {
                host_link::request_find_file(this.tx.clone(), String::new());
            } else {
                this.sov.quickopen_input = None;
                this.sov.quickopen_sub = None;
                this.sov.quickopen_results.clear();
            }
            cx.notify();
        }))
        .on_action(cx.listener(|this, _: &ToggleFindInFiles, _, cx| {
            this.sov.fif_open = !this.sov.fif_open;
            if !this.sov.fif_open {
                this.sov.fif_input = None;
                this.sov.fif_sub = None;
                this.sov.fif_results.clear();
                this.sov.fif_query_len = 0;
                this.sov.fif_busy = false;
            }
            cx.notify();
        }))
        .on_action(cx.listener(|this, _: &ToggleWorkspaceSymbols, _, cx| {
            this.sov.ws_open = !this.sov.ws_open;
            if !this.sov.ws_open {
                this.close_ws();
            }
            cx.notify();
        }))
        .on_action(
            cx.listener(|this, _: &crate::actions::SaveFile, window, cx| {
                this.save_editor(window, cx);
            }),
        )
        .on_action(cx.listener(|this, _: &crate::actions::UndoFileOp, _, cx| {
            this.undo_fs();
            cx.notify();
        }))
        // Contributed keybindings после action-биндингов и фокусных инпутов.
        .capture_key_down(|_, _, _| crate::web::frame_rhythm::input("input/key"))
        .on_key_down(cx.listener(|this, ev: &gpui::KeyDownEvent, _, cx| {
            if this.tree_key(ev, cx) {
                return;
            }
            let norm = crate::contrib_keys::normalize_keystroke(&ev.keystroke);
            if let Some(cmd) = this.contrib_keys.get(&norm).cloned() {
                cx.stop_propagation();
                std::thread::spawn(move || {
                    if let Some(c) = crate::host_link::client() {
                        let _ = c.request("kamin:command:execute", vec![serde_json::json!(cmd)]);
                    }
                });
            }
        }))
        .on_action(
            cx.listener(|this, _: &crate::actions::RenameActiveSession, _, cx| {
                // `onKeyDown` висит на САМОЙ строке (`SessionItem.tsx:98`):
                // F2 переименовывает строку ПОД ФОКУСОМ, а не активную
                // сессию. Фокуса на строке нет — F2 не делает ничего
                if let Some(id) = crate::ui::focus_ring::focused_id()
                    .and_then(|f| f.strip_prefix("srow:").map(str::to_string))
                {
                    let _ = this.tx.try_send(ShellEvent::BeginRename(id));
                    cx.notify();
                }
            }),
        )
        .on_action(
            cx.listener(|this, _: &crate::actions::ToggleSidebarAction, _, cx| {
                this.sidebar_visible = !this.sidebar_visible;
                crate::layout_store::save_patch(
                    serde_json::json!({"sidebarVisible": this.sidebar_visible}),
                );
                cx.notify();
            }),
        )
        // Enter в инпут-оверлеях: их собственные `on_key_down` висят на
        // элементах OVERLAY-окна и клавиатуру не получают (фокус инпута в
        // main), поэтому действие обрабатываем здесь — по тому же
        // приоритету оверлеев, что и Escape.
        .on_action(cx.listener(Self::press_enter))
        .on_action(cx.listener(|_, _: &crate::actions::FocusNext, window, cx| {
            window.focus_next(cx);
            cx.notify();
        }))
        .on_action(
            cx.listener(|_, _: &crate::actions::FocusPrevious, window, cx| {
                window.focus_prev(cx);
                cx.notify();
            }),
        )
        .on_action(cx.listener(|this, _: &CloseOverlay, _, cx| {
            if let Some(qp) = this.quick_pick.take() {
                this.qp_input = None;
                let _ = this
                    .tx
                    .try_send(ShellEvent::QuickPickResolve(qp.req_id, None));
                cx.notify();
                return;
            }
            if this.sov.ws_open {
                this.sov.ws_open = false;
                this.close_ws();
            } else if this.sov.fif_open {
                this.sov.fif_open = false;
                this.sov.fif_input = None;
                this.sov.fif_sub = None;
                this.sov.fif_results.clear();
                this.sov.fif_query_len = 0;
                this.sov.fif_busy = false;
            } else if this.sov.quickopen_open {
                this.sov.quickopen_open = false;
                this.sov.quickopen_input = None;
                this.sov.quickopen_sub = None;
                this.sov.quickopen_results.clear();
            } else if this.palette_open {
                this.palette_open = false;
                this.palette_input = None;
                this.palette_sub = None;
            } else if this.modal.is_some() {
                this.modal = None;
            } else if this.session_menu.is_some() {
                this.session_menu = None;
            } else if this.file_menu.is_some() {
                this.file_menu = None;
            } else if this.tool_picker.is_some() {
                this.tool_picker = None;
            } else if this.tool_tab_menu.is_some() {
                // Escape закрывает контекст-меню тула вместе с сабменю
                this.tool_tab_menu = None;
                this.tool_menu_sub = false;
            } else if this.term.term_menu_open {
                this.term.term_menu_open = false;
            } else if this.new_session_menu.is_some() {
                this.new_session_menu = None;
            } else if this.tabs_overflow_open.is_some() {
                this.tabs_overflow_open = None;
            } else if this.ed.editor_tab_menu.is_some() {
                this.ed.editor_tab_menu = None;
            } else if this.ed.file_tabs_overflow_open {
                this.ed.file_tabs_overflow_open = false;
            } else if this.layout_popover || this.appearance_popover {
                this.layout_popover = false;
                this.appearance_popover = false;
            } else if this.renaming_session.is_some() {
                this.renaming_session = None;
                this.rename_input = None;
            }
            cx.notify();
        }))
    }
}
