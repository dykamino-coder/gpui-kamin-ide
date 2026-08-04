//! Редактор: вкладки, сохранение, диагностика, quick-open
//! и файловые операции (удаление в корзину, перечитывание изменённых).
//!
//! Кого сюда зовут — решает `state/events/dispatch.rs`; связку
//! «вариант события → модуль» проверяет `scripts/check_event_routing.py`.

use crate::file_names::norm_path;
use crate::host::events::CzEvent;
use crate::host::events::EdEvent;
use crate::host::events::TreeEvent;
use crate::host_link::{self, ShellEvent};
use crate::ui::modal::{Modal, ModalAction};
use gpui::Context;

use crate::root::RootView;

impl RootView {
    /// Редактор: вкладки, сохранение, диагностика, quick-open.
    pub(crate) fn apply_editor(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::OpenDevtools(view_id) => {
                crate::web::open_devtools(&view_id);
            }
            ShellEvent::Cz(CzEvent::DiagSnapshot(v)) => {
                self.diags.clear();
                if let Some(arr) = v.as_array() {
                    for e in arr {
                        self.apply_diag_set(e);
                    }
                }
            }
            ShellEvent::Cz(CzEvent::DiagSet(v)) => self.apply_diag_set(&v),
            ShellEvent::OpenToolTabMenu(slot, id, x, y) => {
                self.close_popovers_except("ttab");
                // Каждое открытие начинается со свёрнутого сабменю
                // (`ActivityContextMenu.tsx:65`); клик-мимо гасил только само
                // меню, и следующий RMB показывал сабменю у старого якоря
                self.tool_menu_sub = false;
                self.tool_tab_menu = Some((slot, id, x, y));
            }
            ShellEvent::Ed(EdEvent::OpenInTerminal(dir)) => {
                // Терминал-тул в mainBottom + новый шелл в каталоге узла
                use crate::activity::PanelSlot;
                self.activity.pin(PanelSlot::MainBottom, "terminal");
                self.activity.set_active(PanelSlot::MainBottom, "terminal");
                self.save_activity();
                self.spawn_shell_in(&crate::term::profiles()[0], Some(dir));
            }
            ShellEvent::OpenModal(modal) => {
                // Запоминаем, кто был в фокусе, и просим автофокус Confirm
                self.modal_focus_return = crate::ui::focus_ring::focused_id();
                self.modal_autofocus_pending = true;
                self.modal = Some(modal);
                self.modal_at = Some(std::time::Instant::now());
            }
            ShellEvent::OpenToolPicker(slot, x, y, up) => {
                self.close_popovers_except("picker");
                self.tool_picker = Some((slot, x, y, up));
            }
            ShellEvent::Cz(CzEvent::OpenCustomizePanel(id)) => {
                if !self.cz.customize_open {
                    let _ = self.tx.try_send(ShellEvent::Cz(CzEvent::ToggleCustomize));
                }
                let _ = self
                    .tx
                    .try_send(ShellEvent::Cz(CzEvent::SetCustomizePanel(id)));
            }
            ShellEvent::Ed(EdEvent::CloseEditorTab(i)) => self.close_editor_tab(i),
            ShellEvent::Ed(EdEvent::SelectEditorTab(i)) => {
                if i < self.ed.editor_tabs.len() {
                    self.ed.editor_active = i;
                    self.ed.tabs_reveal_active = true;
                }
            }
            ShellEvent::Ed(EdEvent::CloseOtherEditorTabs(i)) => {
                if i < self.ed.editor_tabs.len() {
                    let keep = self.ed.editor_tabs.swap_remove(i);
                    self.ed.editor_tabs.clear();
                    self.ed.editor_tabs.push(keep);
                    self.ed.editor_active = 0;
                    self.ed.tabs_reveal_active = true;
                }
            }
            ShellEvent::Ed(EdEvent::CloseEditorTabsRight(i)) => {
                if i < self.ed.editor_tabs.len() {
                    self.ed.editor_tabs.truncate(i + 1);
                    self.ed.editor_active = self.ed.editor_active.min(i);
                    self.ed.tabs_reveal_active = true;
                }
            }
            ShellEvent::Ed(EdEvent::CloseAllEditorTabs) => {
                self.ed.editor_tabs.clear();
                self.ed.editor_active = 0;
                self.ed.tabs_reveal_active = true;
            }
            ShellEvent::Ed(EdEvent::OpenEditorTabMenu(index, path, x, y)) => {
                self.close_popovers_except("etab");
                let pinned = self.ed.editor_tabs.get(index).is_some_and(|t| t.pinned);
                self.ed.editor_tab_menu = Some(crate::ui::editor_tabs::EditorTabMenu {
                    index,
                    path,
                    x,
                    y,
                    pinned,
                });
            }
            ShellEvent::Ed(EdEvent::TogglePinEditorTab(i)) => {
                if let Some(tab) = self.ed.editor_tabs.get_mut(i) {
                    tab.pinned = !tab.pinned;
                    self.sort_tabs_pinned_first();
                }
            }
            ShellEvent::Ed(EdEvent::CloseEditorTabMenu) => self.ed.editor_tab_menu = None,
            ShellEvent::OpenSaveLayoutPrompt => {
                self.layout_popover = false;
                self.modal = Some(Modal {
                    title: "Save layout".into(),
                    body: "".into(),
                    confirm_label: "OK".into(),
                    danger: false,
                    prompt: Some(String::new()),
                    placeholder: Some("Layout name".into()),
                    validate: None,
                    cancel_label: None,
                    action: ModalAction::SaveLayoutPreset,
                });
            }
            ShellEvent::ToggleQuickOpen => {
                self.sov.quickopen_open = !self.sov.quickopen_open;
                if self.sov.quickopen_open {
                    host_link::request_find_file(self.tx.clone(), String::new());
                } else {
                    self.sov.quickopen_input = None;
                    self.sov.quickopen_sub = None;
                    self.sov.quickopen_results.clear();
                }
            }
            ShellEvent::CloseQuickOpen => {
                self.sov.quickopen_open = false;
                self.sov.quickopen_input = None;
                self.sov.quickopen_sub = None;
                self.sov.quickopen_results.clear();
            }
            ShellEvent::QuickOpenResults(hits) => self.sov.quickopen_results = hits,
            ShellEvent::Ed(EdEvent::EditorFind) => self.probe_editor_find = true,
            ShellEvent::Ed(EdEvent::FsDelete(path)) => {
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    match trash::delete(&path) {
                        Ok(()) => {
                            let _ = tx.try_send(ShellEvent::Ed(EdEvent::PushFsUndo(
                                crate::host_link::FsUndo::Delete(path),
                            )));
                        }
                        Err(e) => eprintln!("delete {path} failed: {e}"),
                    }
                    let _ = tx.try_send(ShellEvent::Tree(TreeEvent::RefreshTree));
                });
            }
            ShellEvent::Ed(EdEvent::FilesChanged(paths)) => {
                // Reload ЧИСТЫХ открытых табов при внешнем изменении файла
                for p in paths {
                    let np = norm_path(&p);
                    if self
                        .ed
                        .editor_tabs
                        .iter()
                        .any(|t| !t.dirty && norm_path(&t.path) == np)
                        && !self.ed.pending_reload.iter().any(|x| norm_path(x) == np)
                    {
                        self.ed.pending_reload.push(p);
                    }
                }
            }
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
