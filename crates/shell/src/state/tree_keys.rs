//! Клавиатура дерева файлов.
//!
//! Методы перенесены из `root.rs` дословно (`plan/100-refactor-250.md`).

use crate::host::events::EdEvent;
use crate::host_link::ShellEvent;
use crate::state::model::RootView;
use gpui::Context;
impl RootView {
    /// Клавиатура строк дерева (`file-tree-helpers.tsx:28-41`): Delete, F2,
    /// Ctrl+X/C/V по ВЫДЕЛЕНИЮ. Возвращает true, если событие поглощено.
    pub(crate) fn tree_key(&mut self, ev: &gpui::KeyDownEvent, cx: &mut Context<Self>) -> bool {
        // Ввод текста важнее: пока открыт инпут-оверлей или идёт rename —
        // клавиши дереву не достаются
        if self.modal.is_some()
            || self.renaming_session.is_some()
            || self.palette_open
            || self.sov.quickopen_open
        {
            return false;
        }
        let sel: Vec<String> = self.tree(cx).selected.iter().cloned().collect();
        let Some(first) = sel.first().cloned() else {
            return false;
        };
        let k = ev.keystroke.key.as_str();
        let ctrl = ev.keystroke.modifiers.control;
        // `DirEntry` хранит только имя, поэтому «папка ли» определяем по
        // наличию её листинга в кэше (у файлов его нет)
        let is_dir = self.tree(cx).cache.contains_key(&first);
        match (k, ctrl) {
            ("delete", false) => {
                let action = if sel.len() > 1 {
                    crate::ui::modal::ModalAction::DeleteFsMany { paths: sel }
                } else {
                    crate::ui::modal::ModalAction::DeleteFs {
                        path: first.clone(),
                        is_dir,
                    }
                };
                let name = first
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(&first)
                    .to_string();
                let _ = self
                    .tx
                    .try_send(ShellEvent::OpenModal(crate::ui::modal::Modal {
                        title: "Delete".into(),
                        body: format!("Move <b>{name}</b> to the Recycle Bin?").into(),
                        confirm_label: "Move to Recycle Bin".into(),
                        danger: true,
                        prompt: None,
                        cancel_label: None,
                        placeholder: None,
                        validate: None,
                        action,
                    }));
                cx.notify();
                true
            }
            ("f2", false) => {
                // Prompt-модалка переименования — та же, что в контекст-меню
                let name = first
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(&first)
                    .to_string();
                let _ = self
                    .tx
                    .try_send(ShellEvent::OpenModal(crate::ui::modal::Modal {
                        title: "Rename".into(),
                        body: "New name".into(),
                        confirm_label: "Rename".into(),
                        danger: false,
                        prompt: Some(name.clone()),
                        cancel_label: None,
                        placeholder: Some(name.into()),
                        validate: None,
                        action: crate::ui::modal::ModalAction::RenameFs { path: first },
                    }));
                cx.notify();
                true
            }
            ("x", true) => {
                let _ = self.tx.try_send(ShellEvent::Ed(EdEvent::FsCut(sel)));
                true
            }
            ("c", true) => {
                let _ = self.tx.try_send(ShellEvent::Ed(EdEvent::FsCopy(sel)));
                true
            }
            ("v", true) => {
                let dir = if is_dir {
                    first.clone()
                } else {
                    first
                        .rsplit_once(['/', '\\'])
                        .map(|(d, _)| d.to_string())
                        .unwrap_or_else(|| first.clone())
                };
                let _ = self.tx.try_send(ShellEvent::Ed(EdEvent::FsPaste(dir)));
                true
            }
            _ => false,
        }
    }
}
