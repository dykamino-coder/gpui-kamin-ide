//! Выбор пути в дереве файлов: раскрытие родителей и подгрузка.
//!
//! Методы перенесены из `root.rs` дословно (`plan/100-refactor-250.md`).

use crate::file_names::{file_name_error, is_reserved_name};
use crate::host::events::EdEvent;
use crate::host::events::TreeEvent;
use crate::host_link::{self, ShellEvent};
use crate::state::model::RootView;
use crate::ui::modal::{Modal, ModalAction};
impl RootView {
    /// Выделить путь в дереве одиночным выделением (как клик по строке).
    /// Оригинал держит это эффектом `installActiveFileSelectionSync`.
    pub(crate) fn select_tree_path(&mut self, path: &str, cx: &mut gpui::App) {
        // Явный мультиселект (Ctrl/Shift) синк НЕ трогает: жест не меняет
        // `selectedFile`, поэтому эффект оригинала его не перебивает
        // (`file-selection.ts:51-56`) — иначе Shift-диапазон схлопывался
        // обратно в одну строку (ревью ц.23)
        if self.tree(cx).selected.len() > 1 {
            return;
        }
        if self.tree(cx).selected.len() == 1
            && self
                .tree(cx)
                .selected
                .iter()
                .any(|s| crate::ui::file_list::same_path(s, path))
        {
            return;
        }
        self.tree_mut(cx, |tree| {
            tree.selected.clear();
            tree.selected.insert(path.to_string());
        });
    }

    pub(crate) fn run_modal_action(&mut self, modal: Modal, input: Option<String>) {
        let tx = self.tx.clone();
        match modal.action {
            // Демо-модалки Design-панели: подтверждение ничего не делает
            // (в оригинале showConfirm/showPrompt просто резолвят промис).
            ModalAction::Noop => {}
            ModalAction::RemoveLegacyBridge => {
                // Порядок оригинала: СНАЧАЛА реимпорт сессий (пока конфиг жив),
                // затем uninstall. Блокирующий `cmd /C <uninstaller>` уводим в
                // поток — иначе окно висит на время удаления.
                self.cz.legacy_removing = true;
                std::thread::spawn(|| {
                    if let Some(c) = crate::host_link::client() {
                        let _ = c.request("kamin:bridge:reimportSessions", vec![]);
                    }
                    match crate::legacy_bridge::uninstall_electron_bridge() {
                        Ok(report) => eprintln!("legacy bridge removed: {report}"),
                        Err(e) => eprintln!("legacy bridge removal failed: {e}"),
                    }
                });
            }
            ModalAction::DeleteSession(id) => {
                std::thread::spawn(move || {
                    if let Some(client) = host_link::client() {
                        let _ =
                            client.request("kamin:sessions:delete", vec![serde_json::json!(id)]);
                    }
                });
            }
            ModalAction::DeleteProject(id) => {
                std::thread::spawn(move || {
                    if let Some(client) = host_link::client() {
                        let _ = client
                            .request("kamin:sessions:deleteProject", vec![serde_json::json!(id)]);
                    }
                });
            }
            ModalAction::CreateEntry { dir, folder } => {
                let Some(name) = input
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                else {
                    return;
                };
                if let Some(err) = file_name_error(&name) {
                    let _ = tx.try_send(ShellEvent::Toast(crate::ui::toasts::Toast {
                        id: "fs-name-invalid".into(),
                        severity: "warning".into(),
                        title: None,
                        message: err.into(),
                        actions: Vec::new(),
                        sticky: false,
                    }));
                    return;
                }
                std::thread::spawn(move || {
                    let path = std::path::Path::new(&dir).join(&name);
                    let res = if folder {
                        std::fs::create_dir_all(&path)
                    } else {
                        std::fs::write(&path, b"")
                    };
                    match res {
                        Ok(()) => {
                            let _ = tx.try_send(ShellEvent::Ed(EdEvent::PushFsUndo(
                                crate::host_link::FsUndo::Create(path.display().to_string()),
                            )));
                        }
                        Err(e) => eprintln!("create {} failed: {e}", path.display()),
                    }
                    let _ = tx.try_send(ShellEvent::Tree(TreeEvent::RefreshTree));
                });
            }
            ModalAction::RenameFs { path } => {
                let Some(name) = input
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                else {
                    return;
                };
                if let Some(err) = file_name_error(&name) {
                    let _ = tx.try_send(ShellEvent::Toast(crate::ui::toasts::Toast {
                        id: "fs-name-invalid".into(),
                        severity: "warning".into(),
                        title: None,
                        message: err.into(),
                        actions: Vec::new(),
                        sticky: false,
                    }));
                    return;
                }
                std::thread::spawn(move || {
                    let src = std::path::PathBuf::from(&path);
                    let dst = src
                        .parent()
                        .map(|d| d.join(&name))
                        .unwrap_or_else(|| name.into());
                    match std::fs::rename(&src, &dst) {
                        Ok(()) => {
                            let _ = tx.try_send(ShellEvent::Ed(EdEvent::PushFsUndo(
                                crate::host_link::FsUndo::Rename {
                                    from: src.display().to_string(),
                                    to: dst.display().to_string(),
                                },
                            )));
                        }
                        Err(e) => eprintln!("rename {} failed: {e}", src.display()),
                    }
                    let _ = tx.try_send(ShellEvent::Tree(TreeEvent::RefreshTree));
                });
            }
            ModalAction::RenamePreset { old } => {
                let Some(name) = input
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                else {
                    return;
                };
                let mut presets = crate::layout_store::load_presets();
                if let Some(p) = presets.iter_mut().find(|p| p.name == old) {
                    p.name = name;
                    crate::layout_store::save_presets(&presets);
                }
            }
            ModalAction::SaveLayoutPreset => {
                let Some(name) = input
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                else {
                    return;
                };
                if let Ok(snapshot) = serde_json::to_value(&self.layout) {
                    let mut presets = crate::layout_store::load_presets();
                    presets.retain(|p| p.name != name); // overwrite same-name
                    presets.push(crate::layout_store::LayoutPreset {
                        name,
                        snapshot,
                        default: false,
                    });
                    crate::layout_store::save_presets(&presets);
                }
            }
            ModalAction::DeleteFs { path, is_dir: _ } => {
                // В корзину (восстановимо через Ctrl+Z), а зарезервированное
                // имя — навсегда: корзина его не принимает (`deleteEntry`)
                let name = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string();
                if is_reserved_name(&name) {
                    let _ = tx.try_send(ShellEvent::Ed(EdEvent::FsDeletePermanent(path)));
                } else {
                    let _ = tx.try_send(ShellEvent::Ed(EdEvent::FsDelete(path)));
                }
            }
            ModalAction::DeleteFsMany { paths } => {
                for path in paths {
                    let _ = tx.try_send(ShellEvent::Ed(EdEvent::FsDelete(path)));
                }
            }
        }
    }
}
