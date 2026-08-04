//! Сохранение файла редактора и персист открытых файлов.
//!
//! Методы перенесены из `root.rs` дословно (`plan/100-refactor-250.md`).

use crate::file_names::basename_of;
use crate::host::events::TreeEvent;
use crate::host_link::ShellEvent;
use crate::state::fs_ops::restore_from_trash;
use crate::state::model::RootView;
use gpui::{Context, Window};
impl RootView {
    /// Персист открытых файлов (открыть заново при старте).
    pub(crate) fn persist_open_files(&self) {
        let paths: Vec<serde_json::Value> = self
            .ed
            .editor_tabs
            .iter()
            .map(|t| serde_json::Value::String(t.path.clone()))
            .collect();
        crate::layout_store::save_patch(serde_json::json!({ "openFiles": paths }));
    }

    pub(crate) fn close_editor_tab(&mut self, i: usize) {
        if i < self.ed.editor_tabs.len() {
            crate::editor_lsp::HostLsp::close(self.ed.editor_tabs[i].path.clone());
            self.ed.editor_tabs.remove(i);
            self.persist_open_files();
            if self.ed.editor_active >= self.ed.editor_tabs.len() && self.ed.editor_active > 0 {
                self.ed.editor_active = self.ed.editor_tabs.len() - 1;
                self.ed.tabs_reveal_active = true;
            }
        }
    }

    /// Сохранить файл АКТИВНОГО таба (Ctrl+S / кнопка Save).
    pub(crate) fn save_editor(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.ed.editor_tabs.get(self.ed.editor_active) else {
            return;
        };
        let (input, path) = (&tab.input, tab.path.clone());
        let text = input.read(cx).value().to_string();
        match std::fs::write(&path, text) {
            Ok(()) => {
                if let Some(tab) = self.ed.editor_tabs.get_mut(self.ed.editor_active) {
                    tab.dirty = false;
                }
                let name = path
                    .replace('\\', "/")
                    .rsplit('/')
                    .next()
                    .unwrap_or(&path)
                    .to_string();
                self.push_syslog("info", "editor", &format!("Saved {name}"));
            }
            Err(e) => {
                self.push_syslog("error", "editor", &format!("Save failed: {e}"));
                let _ = self
                    .tx
                    .try_send(ShellEvent::Toast(crate::ui::toasts::Toast {
                        id: "save-failed".into(),
                        severity: "error".into(),
                        title: None,
                        message: format!("Save failed: {e}"),
                        actions: Vec::new(),
                        sticky: false,
                    }));
            }
        }
        cx.notify();
    }

    /// Pinned-табы всегда слева (stable sort — внутренний порядок цел);
    /// активный таб следует за содержимым.
    pub(crate) fn sort_tabs_pinned_first(&mut self) {
        let active_path = self
            .ed
            .editor_tabs
            .get(self.ed.editor_active)
            .map(|t| t.path.clone());
        self.ed.editor_tabs.sort_by_key(|t| !t.pinned);
        if let Some(p) = active_path
            && let Some(i) = self.ed.editor_tabs.iter().position(|t| t.path == p)
        {
            self.ed.editor_active = i;
            self.ed.tabs_reveal_active = true;
        }
    }

    /// Ctrl+Z: откат последней файл-операции (инверсия в фоне + тост).
    pub(crate) fn undo_fs(&mut self) {
        use crate::host_link::FsUndo;
        let Some(op) = self.fs_undo.pop() else {
            return;
        };
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let res: Result<String, String> = match op {
                FsUndo::Create(p) => trash::delete(&p)
                    .map(|_| format!("Removed {}", basename_of(&p)))
                    .map_err(|e| e.to_string()),
                FsUndo::Rename { from, to } => std::fs::rename(&to, &from)
                    .map(|_| format!("Renamed back to {}", basename_of(&from)))
                    .map_err(|e| e.to_string()),
                FsUndo::Delete(p) => {
                    restore_from_trash(&p).map(|_| format!("Restored {}", basename_of(&p)))
                }
                FsUndo::Paste { dst, cut_src } => match cut_src {
                    Some(src) => std::fs::rename(&dst, &src)
                        .map(|_| format!("Moved back {}", basename_of(&src)))
                        .map_err(|e| e.to_string()),
                    None => trash::delete(&dst)
                        .map(|_| format!("Removed {}", basename_of(&dst)))
                        .map_err(|e| e.to_string()),
                },
            };
            let (severity, message) = match res {
                Ok(msg) => ("success", msg),
                Err(e) => ("error", format!("Undo failed: {e}")),
            };
            let _ = tx.try_send(ShellEvent::Toast(crate::ui::toasts::Toast {
                id: "fs-undo".into(),
                severity: severity.into(),
                title: None,
                message,
                actions: Vec::new(),
                sticky: false,
            }));
            let _ = tx.try_send(ShellEvent::Tree(TreeEvent::RefreshTree));
        });
    }

    /// Применить kamin:diag:set / элемент снапшота: {owner, uri, diagnostics};
    /// пустой список диагностик = снять ключ.
    pub(crate) fn apply_diag_set(&mut self, v: &serde_json::Value) {
        let owner = v
            .get("owner")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let uri = v
            .get("uri")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if uri.is_empty() {
            return;
        }
        let list: Vec<crate::ui::problems::Diag> = v
            .get("diagnostics")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(crate::ui::problems::Diag::from_value)
                    .collect()
            })
            .unwrap_or_default();
        if list.is_empty() {
            self.diags.remove(&(owner, uri));
        } else {
            self.diags.insert((owner, uri), list);
        }
    }

    /// Persist панельной модели (после pin/unpin/активации/переноса).
    pub(crate) fn save_activity(&self) {
        crate::layout_store::save_patch(
            serde_json::json!({ "activityModel": self.activity.to_json() }),
        );
    }

    /// Индекс тула в pinned-списке слота (для само-ховера при tool-dnd).
    pub(crate) fn tab_index_of(&self, slot: crate::activity::PanelSlot, id: &str) -> Option<usize> {
        self.activity
            .state(slot)
            .pinned
            .iter()
            .position(|t| t == id)
    }
}
