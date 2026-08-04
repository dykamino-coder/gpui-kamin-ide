//! Модель модалки: данные и действия.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use gpui::SharedString;

/// Что показывает модалка: подтверждение или prompt (с текстовым полем).
#[derive(Clone)]
pub struct Modal {
    pub title: SharedString,
    pub body: SharedString,
    pub confirm_label: SharedString,
    pub danger: bool,
    /// Some(seed) → prompt-режим: инпут (создаёт overlay-окно), значение
    /// уезжает в ConfirmModalInput.
    pub prompt: Option<String>,
    /// Подпись кнопки отмены (`cancelLabel`; по умолчанию «Cancel»).
    pub cancel_label: Option<SharedString>,
    /// Подсказка инпута prompt-режима (`PromptModal placeholder`).
    pub placeholder: Option<SharedString>,
    /// Валидатор значения (`validate` оригинала): Some(текст) → строка ошибки
    /// под полем и заблокированный OK. Гоняется на каждый ввод.
    pub validate: Option<fn(&str) -> Option<&'static str>>,
    pub action: ModalAction,
}
/// Что сделать при подтверждении (маршрутизируется в RootView::run_modal_action).
#[derive(Clone, PartialEq)]
pub enum ModalAction {
    /// Демо-модалка Design-панели — подтверждение без побочных эффектов.
    Noop,
    /// Снести старый Electron-Bridge (реимпорт сессий → uninstall).
    RemoveLegacyBridge,
    DeleteSession(String),
    DeleteProject(String),
    /// Создать файл/папку в dir (имя — из prompt-инпута).
    CreateEntry {
        dir: String,
        folder: bool,
    },
    /// Переименовать файл/папку (новое имя — из prompt-инпута).
    RenameFs {
        path: String,
    },
    /// Удалить файл/папку с диска.
    DeleteFs {
        path: String,
        is_dir: bool,
    },
    /// Сохранить текущий layout как пресет (имя — из prompt-инпута).
    SaveLayoutPreset,
    /// Переименовать пресет (новое имя — из prompt-инпута).
    RenamePreset {
        old: String,
    },
    /// Удалить НЕСКОЛЬКО путей (мультиселект дерева) — все в корзину.
    DeleteFsMany {
        paths: Vec<String>,
    },
}
